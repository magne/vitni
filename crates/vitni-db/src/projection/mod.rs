//! The conclusion projections' writer (ADR 0050): one view row per aggregate, holding the serialized
//! view and the sequence of the last event folded into it.
//!
//! `cqrs-es` commits a command's events before it dispatches them to the queries, so a process killed
//! in between leaves a view without its newest events. The write journal names that aggregate, and the
//! next open replays its whole log through the same queries (`catch_up_journal` in the engine
//! modules). Because a view row knows the last event it holds, a replay folds only the events after it:
//! replaying never applies an event twice, so the catch-up repairs a view in place.
//!
//! A write is conditional on the sequence it read, so two writers of one view — a command and another
//! process catching up the same aggregate — cannot both land: the loser reads the view again and folds
//! what is still missing.

use std::marker::PhantomData;

use async_trait::async_trait;
use cqrs_es::{Aggregate, EventEnvelope, Query, View};

use crate::store::{DbError, deserialize_view};

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

/// How many times a write that lost to another writer reads the view again before it gives up.
const WRITE_ATTEMPTS: usize = 5;

/// A batch that starts past the event after the view's last: the view is missing events the batch
/// does not carry, so folding it would skip them.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("the view holds events up to {applied}, but the next one given is {found}")]
pub(crate) struct Gap {
    /// The sequence of the last event the view holds.
    pub(crate) applied: usize,
    /// The sequence of the first event the batch adds.
    pub(crate) found: usize,
}

/// Folds into `view` the events of `events` after sequence `applied`, and returns the sequence of the
/// last one folded — `None` when the view already held them all.
///
/// # Errors
///
/// [`Gap`] when the first new event is not the one right after `applied`; `view` is then untouched.
pub(crate) fn fold<A: Aggregate, V: View<A>>(
    view: &mut V,
    applied: usize,
    events: &[EventEnvelope<A>],
) -> Result<Option<usize>, Gap> {
    let mut last = applied;
    for event in events {
        if event.sequence <= last {
            continue;
        }
        if event.sequence != last + 1 {
            return Err(Gap {
                applied: last,
                found: event.sequence,
            });
        }
        last = event.sequence;
    }
    if last == applied {
        return Ok(None);
    }
    for event in events {
        if event.sequence > applied {
            view.update(event);
        }
    }
    Ok(Some(last))
}

/// One stored view row: the serialized view and the sequence of the last event folded into it.
pub(crate) struct StoredView {
    /// The serialized view, as the engine returns it.
    pub(crate) payload: String,
    /// The sequence of the last event folded into it.
    pub(crate) sequence: usize,
}

/// Whether a conditional view write landed.
pub(crate) enum Written {
    /// The row now holds the new view.
    Landed,
    /// Another writer changed the row since it was read; nothing was written.
    Lost,
}

/// A stored sequence as the event envelope's, failing on a negative one.
fn sequence_from(table: &str, view_id: &str, stored: i64) -> Result<usize, DbError> {
    usize::try_from(stored).map_err(|_| DbError::Backend(format!("{table} {view_id} holds sequence {stored}")))
}

/// An event envelope's sequence as the stored column.
fn sequence_to(sequence: usize) -> Result<i64, DbError> {
    i64::try_from(sequence).map_err(|_| DbError::Backend(format!("sequence {sequence} does not fit the column")))
}

/// An engine's view table rows, read and written by [`ProjectionQuery`].
#[async_trait]
pub(crate) trait ViewRows: Send + Sync {
    /// The row of `view_id` in `table`, if any.
    async fn load(&self, table: &str, view_id: &str) -> Result<Option<StoredView>, DbError>;

    /// Writes `payload` at `sequence`, provided the row still holds `read` — the sequence it was
    /// read at, or `None` when it was absent.
    async fn write(
        &self,
        table: &str,
        view_id: &str,
        payload: serde_json::Value,
        sequence: usize,
        read: Option<usize>,
    ) -> Result<Written, DbError>;
}

/// A `cqrs-es` query that folds an aggregate's events into its view row in `table`, skipping the
/// events the row already holds. Its failures are reported through [`crate::projection_failures`].
pub(crate) struct ProjectionQuery<V, A, R> {
    rows: R,
    table: &'static str,
    _view: PhantomData<fn() -> (V, A)>,
}

impl<V, A, R> ProjectionQuery<V, A, R> {
    /// The query writing `table` through `rows`.
    pub(crate) fn new(rows: R, table: &'static str) -> Self {
        Self {
            rows,
            table,
            _view: PhantomData,
        }
    }
}

impl<V: View<A>, A: Aggregate, R: ViewRows> ProjectionQuery<V, A, R> {
    async fn apply(&self, view_id: &str, events: &[EventEnvelope<A>]) -> Result<(), DbError> {
        for _ in 0..WRITE_ATTEMPTS {
            let stored = self.rows.load(self.table, view_id).await?;
            let (mut view, read) = match stored {
                Some(stored) => (
                    deserialize_view::<V>(self.table, &stored.payload)?,
                    Some(stored.sequence),
                ),
                None => (V::default(), None),
            };
            let folded = fold(&mut view, read.unwrap_or(0), events)
                .map_err(|gap| DbError::Backend(format!("updating {view_id}: {gap}")))?;
            let Some(sequence) = folded else {
                return Ok(());
            };
            let payload =
                serde_json::to_value(&view).map_err(|e| DbError::Backend(format!("encoding {view_id}: {e}")))?;
            match self.rows.write(self.table, view_id, payload, sequence, read).await? {
                Written::Landed => return Ok(()),
                Written::Lost => {}
            }
        }
        Err(DbError::Backend(format!(
            "updating {view_id}: other writers changed it {WRITE_ATTEMPTS} times in a row"
        )))
    }
}

#[async_trait]
impl<V: View<A>, A: Aggregate, R: ViewRows> Query<A> for ProjectionQuery<V, A, R> {
    async fn dispatch(&self, view_id: &str, events: &[EventEnvelope<A>]) {
        if let Err(error) = self.apply(view_id, events).await {
            crate::projection_failures::report(self.table, &error);
        }
    }
}

/// Dispatches to each of its queries in order, as a `CqrsFramework` does — so a replay runs through
/// exactly the queries a command does.
pub(crate) struct QueryChain<A: Aggregate>(pub(crate) Vec<Box<dyn Query<A>>>);

#[async_trait]
impl<A: Aggregate> Query<A> for QueryChain<A> {
    async fn dispatch(&self, aggregate_id: &str, events: &[EventEnvelope<A>]) {
        for query in &self.0 {
            query.dispatch(aggregate_id, events).await;
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use cqrs_es::doc::{Customer, CustomerEvent};
    use cqrs_es::{EventEnvelope, View};
    use serde::{Deserialize, Serialize};

    use super::{Gap, fold};

    #[derive(Debug, Default, Serialize, Deserialize, PartialEq, Eq)]
    struct Names(Vec<String>);

    impl View<Customer> for Names {
        fn update(&mut self, event: &EventEnvelope<Customer>) {
            if let CustomerEvent::NameAdded { name } = &event.payload {
                self.0.push(name.clone());
            }
        }
    }

    fn named(sequences: &[usize]) -> Vec<EventEnvelope<Customer>> {
        let mut events = Vec::new();
        for &sequence in sequences {
            events.push(EventEnvelope {
                aggregate_id: "c".to_owned(),
                sequence,
                payload: CustomerEvent::NameAdded {
                    name: format!("n{sequence}"),
                },
                metadata: HashMap::new(),
            });
        }
        events
    }

    fn names(names: &[&str]) -> Names {
        Names(names.iter().map(|name| (*name).to_owned()).collect())
    }

    #[test]
    fn a_fresh_view_folds_every_event() {
        let mut view = Names::default();
        assert_eq!(fold(&mut view, 0, &named(&[1, 2])), Ok(Some(2)));
        assert_eq!(view, names(&["n1", "n2"]));
    }

    #[test]
    fn events_the_view_holds_are_skipped() {
        let mut view = names(&["n1", "n2"]);
        assert_eq!(fold(&mut view, 2, &named(&[1, 2, 3])), Ok(Some(3)));
        assert_eq!(view, names(&["n1", "n2", "n3"]));
    }

    #[test]
    fn a_view_holding_every_event_is_left_alone() {
        let mut view = names(&["n1", "n2"]);
        assert_eq!(fold(&mut view, 2, &named(&[1, 2])), Ok(None));
        assert_eq!(fold(&mut view, 2, &[]), Ok(None));
        assert_eq!(view, names(&["n1", "n2"]));
    }

    #[test]
    fn a_batch_past_the_next_event_is_a_gap_and_folds_nothing() {
        let mut view = names(&["n1"]);
        assert_eq!(fold(&mut view, 1, &named(&[3, 4])), Err(Gap { applied: 1, found: 3 }));
        assert_eq!(view, names(&["n1"]), "a gap folds nothing");
    }

    #[test]
    fn a_gap_inside_the_batch_folds_nothing() {
        let mut view = Names::default();
        assert_eq!(fold(&mut view, 0, &named(&[1, 3])), Err(Gap { applied: 1, found: 3 }));
        assert_eq!(view, Names::default());
    }
}
