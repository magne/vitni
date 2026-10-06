//! Collects the errors `cqrs-es` hands a query's error handler instead of returning them (#486): a
//! failed projection load or write, and an event that fails to read or upcast during a replay.
//!
//! A framework's queries are shared by every command it runs, so a failure is recorded in a slot
//! local to the task that is awaiting [`watch`] — `cqrs-es` dispatches each query in the caller's
//! task — and lands on the command or replay that caused it, never on a concurrent one.

use std::cell::RefCell;
use std::future::Future;

use cqrs_es::persist::{PersistenceError, QueryErrorHandler};

use crate::store::DbError;

tokio::task_local! {
    static FAILURES: RefCell<Option<Failures>>;
}

/// What failed while a [`watch`]ed future ran: the first failure in full, and how many followed it.
#[derive(Debug, PartialEq, Eq)]
pub(crate) struct Failures {
    first: String,
    more: usize,
}

impl Failures {
    fn record(slot: &mut Option<Self>, failure: String) {
        match slot {
            Some(failures) => failures.more += 1,
            None => {
                *slot = Some(Self {
                    first: failure,
                    more: 0,
                });
            }
        }
    }

    /// A [`DbError`] that reads `{context}: {first failure}`, with how many more there were.
    pub(crate) fn into_error(self, context: &str) -> DbError {
        let Self { first, more } = self;
        if more == 0 {
            DbError::Backend(format!("{context}: {first}"))
        } else {
            DbError::Backend(format!("{context}: {first} (and {more} more)"))
        }
    }
}

/// An error handler for a query or replay over `table`. Inside [`watch`] it records the failure for
/// the watcher to report; outside one it logs it, so no failure is ever dropped.
pub(crate) fn handler(table: &'static str) -> Box<QueryErrorHandler> {
    Box::new(move |error: PersistenceError| report(table, &error))
}

/// Records that `table` failed to update with `error` — for a derived index whose query reports its own
/// failures. Inside [`watch`] the watcher reports it; outside one it is logged.
pub(crate) fn report(table: &str, error: &dyn std::fmt::Display) {
    let failure = format!("{table}: {error}");
    let recorded = FAILURES.try_with(|slot| Failures::record(&mut slot.borrow_mut(), failure.clone()));
    if recorded.is_err() {
        tracing::error!(%failure, "a projection failed to update outside any command or replay");
    }
}

/// Runs `future`, returning its output and whatever the [`handler`]s it reached recorded.
pub(crate) async fn watch<F: Future>(future: F) -> (F::Output, Option<Failures>) {
    // On the heap: held inline, a command's future would grow every caller's past the stack.
    let future = Box::pin(future);
    FAILURES
        .scope(RefCell::new(None), async {
            let output = future.await;
            (output, FAILURES.with(RefCell::take))
        })
        .await
}

/// Awaits `replay`, failing it with `context` when it errs or when any event it replayed failed to
/// read or to update its query.
pub(crate) async fn watch_replay<E: std::error::Error>(
    replay: impl Future<Output = Result<(), cqrs_es::AggregateError<E>>>,
    context: &str,
) -> Result<(), DbError> {
    let (result, failures) = watch(replay).await;
    result.map_err(|e| DbError::Backend(format!("{context}: {e}")))?;
    failures.map_or(Ok(()), |failures| Err(failures.into_error(context)))
}

#[cfg(test)]
mod tests {
    use super::{handler, watch};
    use cqrs_es::persist::PersistenceError;

    fn failure() -> PersistenceError {
        PersistenceError::UnknownError("disk full".into())
    }

    #[tokio::test]
    async fn nothing_recorded_watches_as_none() {
        let ((), failures) = watch(async {}).await;
        assert_eq!(failures, None);
    }

    #[tokio::test]
    async fn the_first_failure_is_kept_and_the_rest_counted() {
        let handle = handler("person_view");
        let ((), failures) = watch(async {
            handle(failure());
            handle(failure());
            handle(failure());
        })
        .await;
        let error = failures.expect("recorded").into_error("rebuilding");
        assert_eq!(
            error.to_string(),
            "storage backend error: rebuilding: person_view: disk full (and 2 more)"
        );
    }

    #[tokio::test]
    async fn a_single_failure_reads_without_a_count() {
        let handle = handler("tag_view");
        let ((), failures) = watch(async { handle(failure()) }).await;
        let error = failures.expect("recorded").into_error("committing");
        assert_eq!(
            error.to_string(),
            "storage backend error: committing: tag_view: disk full"
        );
    }

    #[tokio::test]
    async fn a_watch_records_only_its_own_task() {
        let handle = handler("person_view");
        let ((), failures) = watch(async {
            tokio::spawn(async move { handle(failure()) }).await.expect("joined");
        })
        .await;
        assert_eq!(
            failures, None,
            "a failure in another task is logged, not attributed here"
        );
    }
}
