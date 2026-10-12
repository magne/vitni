//! Collects the errors a query reports instead of returning them (#486): `cqrs-es` dispatches to a
//! query without a result, so a failed projection load or write is recorded here for the command or
//! replay that caused it.
//!
//! A framework's queries are shared by every command it runs, so a failure is recorded in a slot
//! local to the task that is awaiting [`watch`] — `cqrs-es` dispatches each query in the caller's
//! task — and lands on the command or replay that caused it, never on a concurrent one.

use std::cell::RefCell;
use std::future::Future;

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

/// Records that `table` failed to update with `error`. Inside [`watch`] the watcher reports it; outside
/// one it is logged, so no failure is ever dropped.
pub(crate) fn report(table: &str, error: &dyn std::fmt::Display) {
    let failure = format!("{table}: {error}");
    let recorded = FAILURES.try_with(|slot| Failures::record(&mut slot.borrow_mut(), failure.clone()));
    if recorded.is_err() {
        tracing::error!(%failure, "a projection failed to update outside any command or replay");
    }
}

/// Runs `future`, returning its output and whatever the [`report`]s it reached recorded.
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

#[cfg(test)]
mod tests {
    use super::{report, watch};

    const FAILURE: &str = "disk full";

    #[tokio::test]
    async fn nothing_recorded_watches_as_none() {
        let ((), failures) = watch(async {}).await;
        assert_eq!(failures, None);
    }

    #[tokio::test]
    async fn the_first_failure_is_kept_and_the_rest_counted() {
        let ((), failures) = watch(async {
            report("person_view", &FAILURE);
            report("person_view", &FAILURE);
            report("person_view", &FAILURE);
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
        let ((), failures) = watch(async { report("tag_view", &FAILURE) }).await;
        let error = failures.expect("recorded").into_error("committing");
        assert_eq!(
            error.to_string(),
            "storage backend error: committing: tag_view: disk full"
        );
    }

    #[tokio::test]
    async fn a_watch_records_only_its_own_task() {
        let ((), failures) = watch(async {
            tokio::spawn(async { report("person_view", &FAILURE) })
                .await
                .expect("joined");
        })
        .await;
        assert_eq!(
            failures, None,
            "a failure in another task is logged, not attributed here"
        );
    }
}
