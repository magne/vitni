//! The `record_origins` projection index (ADR 0037 §4): one row per event an import wrote, keyed by
//! the source record it came from, so a later import of the same record resolves onto the aggregate
//! it made and skips what it already asserted.
//!
//! A row holds the event's origin (`dataset`, `record`, `item`, `run`), the aggregate it landed on,
//! its `AssertionId`, a [`field_key`] naming which field of the aggregate it asserts, the digest of
//! its body, when it was asserted, and whether that assertion is still live. Correction events
//! (`AssertionRetracted`, `AssertionSuperseded`) get no row of their own: they clear `live` on the
//! rows they target. An `ItemResolved` gets a row too, under its payload's dataset, record and item,
//! so an item the user (or deterministic identity) resolved in one run resolves the same way in the
//! next.
//!
//! The field key and digest are derived from the event itself, never stored in its payload, so the
//! index can be dropped and rebuilt by replay (ADR 0010 §5). `vitni-app` derives the same two values
//! from the events a command *would* emit, through the same [`indexed_field`], to decide whether an
//! incoming write is already on record.
//!
//! One engine-neutral half (this module) and one thin submodule per backend, mirroring
//! `place_succession_index`.

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

use std::fmt::Write as _;

use cqrs_es::{Aggregate, EventEnvelope};
use serde::Serialize;
use sha2::{Digest as _, Sha256};
use vitni_core::assertions::{Envelope, EventBody};
use vitni_core::ids::AssertionId;
use vitni_core::origin::ContentDigest;
use vitni_core::provenance::Timestamp;

use crate::store::DbError;

/// The index table.
const RECORD_ORIGINS_TABLE: &str = "record_origins";

/// The two index columns an event's body determines.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct IndexedField {
    /// Which field of the aggregate the event asserts (see [`field_key`]).
    pub field_key: String,
    /// The digest of the event's body.
    pub digest: ContentDigest,
}

/// One indexed assertion, as a lookup returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginRow {
    /// The field the event asserted.
    pub field_key: String,
    /// The aggregate kind it landed on (`Aggregate::TYPE`).
    pub aggregate_kind: String,
    /// The aggregate it landed on.
    pub aggregate_id: String,
    /// The assertion it belongs to.
    pub assertion_id: AssertionId,
    /// The digest of its body.
    pub digest: ContentDigest,
    /// When it was asserted.
    pub occurred_at: Timestamp,
    /// Whether the assertion is still live (neither retracted nor superseded).
    pub live: bool,
}

/// What an item resolved onto, as [`Store::resolve_origin`](crate::Store::resolve_origin) returns it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginResolution {
    /// The aggregate it resolved onto.
    pub aggregate_id: String,
    /// Whether an earlier run of the same dataset created the aggregate from this item (`true`), or
    /// a run recorded resolving the item onto an aggregate another dataset or the user made
    /// (`false`).
    pub created: bool,
}

/// The column values of one row to insert, engine-neutral.
#[derive(Debug)]
pub(crate) struct RowValues {
    pub(crate) dataset: String,
    pub(crate) record: String,
    pub(crate) item: Option<String>,
    pub(crate) field_key: String,
    pub(crate) aggregate_kind: String,
    pub(crate) aggregate_id: String,
    pub(crate) assertion_id: String,
    pub(crate) digest: String,
    pub(crate) run: String,
    pub(crate) occurred_at: String,
}

/// What one committed event means for the index: the row it adds, if any, and whether it is a
/// correction, after which the aggregate's `live` flags are re-read.
#[derive(Debug)]
pub(crate) struct IndexRow {
    pub(crate) origin: Option<RowValues>,
    pub(crate) corrects: bool,
}

/// The index rows a batch of committed events of aggregate `A` adds.
///
/// # Errors
///
/// [`DbError::Backend`] if an event does not serialize, or an `ItemResolved` lacks a field.
pub(crate) fn index_rows<A, B>(aggregate_id: &str, events: &[EventEnvelope<A>]) -> Result<Vec<IndexRow>, DbError>
where
    A: Aggregate<Event = Envelope<B>>,
    B: EventBody + Serialize,
{
    let mut rows = Vec::with_capacity(events.len());
    for envelope in events {
        let event = &envelope.payload;
        let event_type = event.body.type_name();
        let origin = if event_type == "ItemResolved" {
            Some(resolution_row(event)?)
        } else {
            match (&event.context.origin, indexed_field(A::TYPE, &event.body)?) {
                (Some(origin), Some(field)) => Some(RowValues {
                    dataset: origin.dataset.as_str().to_owned(),
                    record: origin.record.clone(),
                    item: origin.item.clone(),
                    field_key: field.field_key,
                    aggregate_kind: A::TYPE.to_owned(),
                    aggregate_id: aggregate_id.to_owned(),
                    assertion_id: event.assertion_id.to_string(),
                    digest: field.digest.as_str().to_owned(),
                    run: origin.run.to_string(),
                    occurred_at: encode_timestamp(event.context.occurred_at)?,
                }),
                _ => None,
            }
        };
        rows.push(IndexRow {
            origin,
            corrects: is_correction(event_type),
        });
    }
    Ok(rows)
}

/// The row an `ItemResolved` adds: filed under its payload's dataset, record and item, pointing at
/// the aggregate it resolved onto.
fn resolution_row<B: EventBody + Serialize>(event: &Envelope<B>) -> Result<RowValues, DbError> {
    let body = serde_json::to_value(&event.body).map_err(|e| DbError::Backend(format!("serializing an event: {e}")))?;
    let text = |field: &str| {
        body.get(field)
            .and_then(serde_json::Value::as_str)
            .map(str::to_owned)
            .ok_or_else(|| DbError::Backend(format!("ItemResolved without `{field}`")))
    };
    let kind = text("kind")?;
    Ok(RowValues {
        dataset: text("dataset")?,
        record: text("record")?,
        item: body.get("item").and_then(serde_json::Value::as_str).map(str::to_owned),
        field_key: resolved_key(&kind),
        aggregate_id: text("aggregate_id")?,
        aggregate_kind: kind,
        assertion_id: event.assertion_id.to_string(),
        digest: digest(&[&event.body])?.as_str().to_owned(),
        run: text("run_id")?,
        occurred_at: encode_timestamp(event.context.occurred_at)?,
    })
}

/// Whether an event type is a correction of an earlier assertion.
fn is_correction(event_type: &str) -> bool {
    matches!(event_type, "AssertionRetracted" | "AssertionSuperseded")
}

fn encode_timestamp(timestamp: Timestamp) -> Result<String, DbError> {
    serde_json::to_string(&timestamp).map_err(|e| DbError::Backend(format!("serializing a timestamp: {e}")))
}

/// Decodes a stored assertion id column.
///
/// # Errors
///
/// [`DbError::Backend`] if it is not a UUID.
pub(crate) fn decode_assertion_id(text: &str) -> Result<AssertionId, DbError> {
    serde_json::from_value(serde_json::Value::String(text.to_owned()))
        .map_err(|e| DbError::Backend(format!("decoding an assertion id: {e}")))
}

/// Decodes a stored `run` column.
///
/// # Errors
///
/// [`DbError::Backend`] if it is not an encoded import run id.
pub(crate) fn decode_run(text: &str) -> Result<vitni_core::ids::ImportRunId, DbError> {
    serde_json::from_value(serde_json::Value::String(text.to_owned()))
        .map_err(|e| DbError::Backend(format!("decoding an import run id: {e}")))
}

/// Decodes a stored `occurred_at` column.
///
/// # Errors
///
/// [`DbError::Backend`] if it is not an encoded timestamp.
pub(crate) fn decode_timestamp(text: &str) -> Result<Timestamp, DbError> {
    serde_json::from_str(text).map_err(|e| DbError::Backend(format!("decoding a timestamp: {e}")))
}

/// The field key and digest of one event body of aggregate `aggregate_type`, or `None` for an event
/// the index does not record (a correction, or an import run's own lifecycle event).
///
/// # Errors
///
/// [`DbError::Backend`] if the body does not serialize.
pub fn indexed_field<B: Serialize>(aggregate_type: &str, body: &B) -> Result<Option<IndexedField>, DbError> {
    let value = serde_json::to_value(body).map_err(|e| DbError::Backend(format!("serializing an event body: {e}")))?;
    let Some(field_key) = field_key(aggregate_type, &value) else {
        return Ok(None);
    };
    Ok(Some(IndexedField {
        field_key,
        digest: digest(&[body])?,
    }))
}

/// The digest of a sequence of event bodies: SHA-256 over each body's JSON encoding, one per line,
/// hex-encoded. The digest of one event is `digest(&[body])`; the digest an assertion's origin
/// carries is the digest of all of its non-correction bodies, in order.
///
/// The encoding is the event encoding itself (ADR 0004 §4), so two bodies digest equal exactly when
/// they would be stored equal.
///
/// # Errors
///
/// [`DbError::Backend`] if a body does not serialize.
pub fn digest<B: Serialize>(bodies: &[&B]) -> Result<ContentDigest, DbError> {
    let mut hasher = Sha256::new();
    for body in bodies {
        let bytes =
            serde_json::to_vec(body).map_err(|e| DbError::Backend(format!("serializing an event body: {e}")))?;
        hasher.update(&bytes);
        hasher.update(b"\n");
    }
    let mut hex = String::with_capacity(64);
    for byte in hasher.finalize() {
        let _ = write!(hex, "{byte:02x}");
    }
    Ok(ContentDigest::new(hex))
}

/// The field an event body asserts, as `<aggregate>.<EventType>` (`event.DateAsserted`,
/// `source.TitleSet`), with the fact type appended for a person fact
/// (`person.FactAsserted.Occupation`), since each fact type is its own field. A creating event is
/// `<aggregate>.created` and an `ItemResolved` is `<kind>.resolved`: the two keys an import resolves
/// an item by. `None` for a correction and for an import run's own lifecycle events.
#[must_use]
pub fn field_key(aggregate_type: &str, body: &serde_json::Value) -> Option<String> {
    let event_type = body.get("type")?.as_str()?;
    match event_type {
        "AssertionRetracted"
        | "AssertionSuperseded"
        | "ImportRunStarted"
        | "ImportRunFinished"
        | "ImportRunAbandoned" => None,
        "ItemResolved" => Some(format!("{}.resolved", body.get("kind")?.as_str()?)),
        "DnaMatchObserved" => Some(format!("{aggregate_type}.created")),
        _ if event_type.ends_with("Created") => Some(format!("{aggregate_type}.created")),
        "FactAsserted" => {
            let fact_type = body.pointer("/fact/fact_type/type")?.as_str()?;
            Some(format!("{aggregate_type}.FactAsserted.{fact_type}"))
        }
        _ => Some(format!("{aggregate_type}.{event_type}")),
    }
}

/// The key of the creating event of an aggregate of `kind` — what an import resolves an item by.
#[must_use]
pub fn created_key(kind: &str) -> String {
    format!("{kind}.created")
}

/// The key of an `ItemResolved` onto an aggregate of `kind`.
#[must_use]
pub fn resolved_key(kind: &str) -> String {
    format!("{kind}.resolved")
}

/// Whether the field `field_key` names holds one value, which a later assertion replaces (an
/// event's date, a source's title), rather than a list each assertion adds to (a person's names, a
/// family's children). Decides how an import reconciles a changed value (ADR 0029).
#[must_use]
pub fn single_valued(field_key: &str) -> bool {
    let Some((aggregate, event_type)) = field_key.split_once('.') else {
        return false;
    };
    if event_type == "RestrictionsChanged" {
        return true;
    }
    let fields: &[&str] = match aggregate {
        "event" => &["EventTypeSet", "DateAsserted", "DescriptionSet", "PlaceLinked"],
        "place" => &["PlaceTypeSet", "CoordinatesAsserted", "CodeSet"],
        "source" => &["TitleSet", "AuthorSet", "PubInfoSet", "AbbrevSet"],
        "citation" => &["PageSet", "DateAsserted", "ConfidenceSet", "EvidenceAnalysisSet"],
        "repository" => &["RepositoryTypeSet", "NameSet"],
        "media" => &["PathSet", "ChecksumSet", "MimeSet", "DateAsserted"],
        "note" => &["NoteTypeSet", "RichTextSet"],
        "tag" => &["TagRenamed", "TagColorSet", "TagPrioritySet"],
        "dna_test" => &["ProviderSet", "KitIdSet", "TestTypeSet", "GenomeBuildSet"],
        "research_note" => &["RichTextSet"],
        _ => &[],
    };
    fields.contains(&event_type)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use uuid::Uuid;
    use vitni_core::assertions::Envelope;
    use vitni_core::enums::FactType;
    use vitni_core::fact::Fact;
    use vitni_core::ids::PersonId;
    use vitni_core::person::event::PersonEventBody;

    use super::{digest, field_key, indexed_field, single_valued};

    fn occupation(value: &str) -> PersonEventBody {
        PersonEventBody::FactAsserted {
            person_id: PersonId::from_uuid(Uuid::from_u128(1)),
            fact: Fact {
                fact_type: FactType::Occupation,
                date: None,
                place_id: None,
                value: Some(value.to_owned()),
            },
        }
    }

    #[test]
    fn a_fact_is_keyed_by_its_fact_type() {
        let field = indexed_field("person", &occupation("Farmer")).unwrap().unwrap();
        assert_eq!(field.field_key, "person.FactAsserted.Occupation");
    }

    #[test]
    fn creating_events_share_one_key_per_aggregate() {
        assert_eq!(
            field_key("place", &json!({"type": "PlaceCreated"})).as_deref(),
            Some("place.created")
        );
        assert_eq!(
            field_key("dna_match", &json!({"type": "DnaMatchObserved"})).as_deref(),
            Some("dna_match.created")
        );
    }

    #[test]
    fn an_item_resolution_is_keyed_by_the_resolved_kind() {
        let body = json!({"type": "ItemResolved", "kind": "source"});
        assert_eq!(field_key("import_run", &body).as_deref(), Some("source.resolved"));
    }

    #[test]
    fn corrections_and_run_lifecycle_events_are_not_indexed() {
        for event_type in [
            "AssertionRetracted",
            "AssertionSuperseded",
            "ImportRunStarted",
            "ImportRunFinished",
            "ImportRunAbandoned",
        ] {
            assert_eq!(
                field_key("person", &json!({ "type": event_type })),
                None,
                "{event_type}"
            );
        }
    }

    #[test]
    fn every_other_person_variant_is_keyed_by_its_event_type() {
        for &variant in Envelope::<PersonEventBody>::variant_names() {
            let key = field_key("person", &json!({ "type": variant }));
            match variant {
                "AssertionRetracted" | "AssertionSuperseded" | "FactAsserted" => {}
                "PersonCreated" => assert_eq!(key.as_deref(), Some("person.created")),
                _ => assert_eq!(key, Some(format!("person.{variant}"))),
            }
        }
    }

    #[test]
    fn equal_bodies_digest_equal_and_different_bodies_differ() {
        let farmer = digest(&[&occupation("Farmer")]).unwrap();
        assert_eq!(farmer, digest(&[&occupation("Farmer")]).unwrap());
        assert_ne!(farmer, digest(&[&occupation("Smith")]).unwrap());
        assert_eq!(farmer.as_str().len(), 64, "hex SHA-256");
    }

    #[test]
    fn a_sequence_digest_depends_on_every_body_and_their_order() {
        let (a, b) = (occupation("Farmer"), occupation("Smith"));
        let ab = digest(&[&a, &b]).unwrap();
        assert_ne!(ab, digest(&[&b, &a]).unwrap());
        assert_ne!(ab, digest(&[&a]).unwrap());
    }

    #[test]
    fn scalar_fields_are_single_valued_and_lists_are_not() {
        for key in [
            "event.DateAsserted",
            "source.TitleSet",
            "citation.PageSet",
            "person.RestrictionsChanged",
        ] {
            assert!(single_valued(key), "{key}");
        }
        for key in [
            "person.NameAsserted",
            "person.SexAsserted",
            "person.FactAsserted.Occupation",
            "place.NameAsserted",
            "family.ChildAdded",
            "event.created",
        ] {
            assert!(!single_valued(key), "{key}");
        }
    }
}
