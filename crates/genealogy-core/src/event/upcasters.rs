//! Event payload upcasters — the schema-evolution registry for the Event aggregate (ADR 0010).
//!
//! An upcaster rewrites a *serialized* historical payload into the current shape at **read time**
//! (aggregate load and projection rebuild); stored events are never mutated (ADR 0001). Each
//! aggregate that has evolved a variant owns its upcasters here, next to the events whose schema
//! they migrate. Versions are per-variant and additive only (ADR 0004 §4): an upcaster fires only
//! when its semantic version *supersedes* the stored `event_version` for that `event_type`.
//!
//! Order matters — push upcasters oldest-first so a payload is migrated through each step in turn
//! (`1.0 → 2.0 → …`).

use cqrs_es::persist::{EventUpcaster, SemanticVersionEventUpcaster};
use serde_json::Value;

/// The ordered upcasters for the Event aggregate.
///
/// Today: `EventCreated` `1.0 → 2.0`, which added the `private` flag (Gramps' universal privacy
/// flag); historical payloads are backfilled with `false`.
#[must_use]
pub fn upcasters() -> Vec<Box<dyn EventUpcaster>> {
    vec![Box::new(SemanticVersionEventUpcaster::new(
        "EventCreated",
        "2.0.0",
        Box::new(add_private_default),
    ))]
}

/// Inserts `private: false` into an `EventCreated` payload that predates the field.
fn add_private_default(payload: Value) -> Value {
    let Value::Object(mut fields) = payload else {
        return payload;
    };
    fields.entry("private").or_insert(Value::Bool(false));
    Value::Object(fields)
}

#[cfg(test)]
mod tests {
    use super::add_private_default;
    use serde_json::json;

    #[test]
    fn backfills_private_when_absent() {
        let v1 = json!({ "type": "EventCreated", "event_id": "x", "human_id": "E1" });
        let upcast = add_private_default(v1);
        assert_eq!(upcast["private"], json!(false));
    }

    #[test]
    fn leaves_an_existing_private_untouched() {
        let v2 = json!({ "type": "EventCreated", "private": true });
        let upcast = add_private_default(v2);
        assert_eq!(upcast["private"], json!(true));
    }
}
