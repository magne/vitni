//! Raw event rows and projection rows — the untyped view of the store a backup reads and a restore
//! writes (ADR 0041).
//!
//! A [`RawEvent`] is one `events` row exactly as stored, all seven columns, so a restore can insert
//! it without re-deciding any command. [`decode_raw_event`] proves a row decodes as its aggregate's
//! event before it is inserted, because a projection rebuild skips an undecodable row silently.
//! [`ProjectionRow`] is one `*_view` row, the unit two stores are compared by.

use cqrs_es::persist::SerializedEvent;
use cqrs_es::{Aggregate, EventEnvelope};

use crate::registry::for_each_db_aggregate;
use crate::store::DbError;

/// One stored event row, every column, engine-neutral.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RawEvent {
    /// The aggregate kind (the stored `Aggregate::TYPE`, e.g. `person`).
    pub aggregate_type: String,
    /// The aggregate instance id (a UUID v7 string).
    pub aggregate_id: String,
    /// The event's position in the aggregate's stream (1-based).
    pub sequence: i64,
    /// The event variant name (e.g. `NameAsserted`).
    pub event_type: String,
    /// The variant's payload schema version (e.g. `1.0`).
    pub event_version: String,
    /// The event payload: the provenance envelope and the body.
    pub payload: serde_json::Value,
    /// The `cqrs-es` metadata map (ops/tracing only — ADR 0004 §1).
    pub metadata: serde_json::Value,
}

impl RawEvent {
    /// The row's primary key, the cursor [`Store::read_raw_events`](crate::Store::read_raw_events)
    /// pages after.
    #[must_use]
    pub fn key(&self) -> RawEventKey {
        RawEventKey {
            aggregate_type: self.aggregate_type.clone(),
            aggregate_id: self.aggregate_id.clone(),
            sequence: self.sequence,
        }
    }
}

/// The `events` primary key, ordered as the store pages it.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct RawEventKey {
    /// The aggregate kind.
    pub aggregate_type: String,
    /// The aggregate instance id.
    pub aggregate_id: String,
    /// The position in the aggregate's stream.
    pub sequence: i64,
}

/// One projection row: which view table, which instance, its replayed event count and payload.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectionRow {
    /// The `*_view` table.
    pub table: String,
    /// The aggregate instance id.
    pub view_id: String,
    /// The number of events applied to the view.
    pub version: i64,
    /// The serialized view.
    pub payload: serde_json::Value,
}

/// Decodes `row` as its aggregate's event, applying that aggregate's upcasters first, exactly as a
/// projection rebuild would.
///
/// # Errors
///
/// [`DbError::Malformed`] naming the row if its aggregate type is unknown, its sequence is not
/// positive, or its payload or metadata does not decode.
pub fn decode_raw_event(row: &RawEvent) -> Result<(), DbError> {
    let sequence = usize::try_from(row.sequence)
        .ok()
        .filter(|sequence| *sequence > 0)
        .ok_or_else(|| malformed(row, "the sequence is not a positive number"))?;
    let serialized = SerializedEvent::new(
        row.aggregate_id.clone(),
        sequence,
        row.aggregate_type.clone(),
        row.event_type.clone(),
        row.event_version.clone(),
        row.payload.clone(),
        row.metadata.clone(),
    );
    decode_serialized(row, serialized)
}

/// Builds the [`DbError::Malformed`] naming `row` and `reason`.
fn malformed(row: &RawEvent, reason: &str) -> DbError {
    DbError::Malformed(format!(
        "{} event {} #{} ({}): {reason}",
        row.aggregate_type, row.aggregate_id, row.sequence, row.event_type
    ))
}

/// Upcasts `serialized` with its aggregate's upcasters and decodes it as that aggregate's event.
fn upcast_and_decode<A: Aggregate>(
    row: &RawEvent,
    mut serialized: SerializedEvent,
    upcasters: &[Box<dyn cqrs_es::persist::EventUpcaster>],
) -> Result<(), DbError> {
    for upcaster in upcasters {
        if upcaster.can_upcast(&serialized.event_type, &serialized.event_version) {
            serialized = upcaster.upcast(serialized);
        }
    }
    EventEnvelope::<A>::try_from(serialized)
        .map(|_| ())
        .map_err(|error| malformed(row, &error.to_string()))
}

/// Generates [`decode_serialized`] and [`event_variants`] from the registry: the `snake` column is
/// the stored `Aggregate::TYPE`.
macro_rules! raw_event_registry {
    ($(($snake:ident, $State:ty, $View:ty, $Cmd:ty, $Err:ty, $table_const:ident, $table_str:literal, $execute:ident, $find:ident, $find_param:ident, $list:ident, $preview:ident, $wiring:tt, $upcasters:expr,)),+ $(,)?) => {
        /// Dispatches `serialized` to its aggregate's decoder by `aggregate_type`.
        fn decode_serialized(row: &RawEvent, serialized: SerializedEvent) -> Result<(), DbError> {
            match row.aggregate_type.as_str() {
                $(stringify!($snake) => upcast_and_decode::<$State>(row, serialized, &$upcasters),)+
                _ => Err(malformed(row, "the aggregate type is not one of the 14 aggregates")),
            }
        }

        /// Every aggregate type with every event variant name it can store, in registry order —
        /// what the current backup fixture must cover (ADR 0041 §5).
        #[must_use]
        pub fn event_variants() -> Vec<(&'static str, &'static [&'static str])> {
            vec![$((<$State as Aggregate>::TYPE, <<$State as Aggregate>::Event>::variant_names()),)+]
        }
    };
}

for_each_db_aggregate!(raw_event_registry);

#[cfg(test)]
mod tests {
    use super::{RawEvent, decode_raw_event, event_variants};

    /// The decoder dispatches on the registry's `snake` name, the variant list on
    /// `Aggregate::TYPE`: every listed kind must reach its decoder, not the unknown-type arm.
    #[test]
    fn every_listed_aggregate_type_has_a_decoder() {
        for (kind, _) in event_variants() {
            let row = RawEvent {
                aggregate_type: kind.to_owned(),
                aggregate_id: "id".to_owned(),
                sequence: 1,
                event_type: "Nothing".to_owned(),
                event_version: "1.0".to_owned(),
                payload: serde_json::json!({}),
                metadata: serde_json::json!({}),
            };
            let Err(error) = decode_raw_event(&row) else {
                continue;
            };
            assert!(!error.to_string().contains("not one of the 14"), "{kind}: {error}");
        }
    }

    #[test]
    fn a_non_positive_sequence_is_malformed() {
        let row = RawEvent {
            aggregate_type: "tag".to_owned(),
            aggregate_id: "id".to_owned(),
            sequence: 0,
            event_type: "TagCreated".to_owned(),
            event_version: "1.0".to_owned(),
            payload: serde_json::json!({}),
            metadata: serde_json::json!({}),
        };
        assert!(decode_raw_event(&row).unwrap_err().to_string().contains("sequence"));
    }
}
