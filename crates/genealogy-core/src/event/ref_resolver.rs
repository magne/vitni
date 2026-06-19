//! The cross-aggregate reference resolver for the Event aggregate (ADR 0004 §3, ADR 0009).
//!
//! Mirrors the Citation resolver: `decide` cannot read the Place projection, so the aggregate-tax
//! check (does the linked place exist?) is resolved before `decide` by the `cqrs-es` `Services`
//! slot. The trait lives here so the aggregate's `Services` type can name it; the SQLite
//! implementation lives in `genealogy-db`.

use async_trait::async_trait;

use crate::event::command::EventCommand;

/// The resolved facts `decide` needs about other aggregates for one Event command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct EventRefs {
    /// Whether the linked `Place` exists in the (possibly-lagging) projection.
    pub place_exists: bool,
}

/// Resolves the cross-aggregate facts for an Event command against the read model.
///
/// Implemented by `genealogy-db` over the Place projection; the `cqrs-es` `Services` value for the
/// Event aggregate is an `Arc<dyn EventRefResolver>`.
#[async_trait]
pub trait EventRefResolver: Send + Sync {
    /// Resolves the [`EventRefs`] for `command` (e.g. whether the linked place exists).
    async fn resolve(&self, command: &EventCommand) -> EventRefs;
}
