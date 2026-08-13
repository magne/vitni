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

use cqrs_es::persist::EventUpcaster;

/// The ordered upcasters for the Event aggregate.
///
/// None today: no Event variant has evolved (workspaces are disposable, so a non-additive schema
/// change recreates rather than migrates).
#[must_use]
pub fn upcasters() -> Vec<Box<dyn EventUpcaster>> {
    Vec::new()
}
