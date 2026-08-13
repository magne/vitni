//! The cross-aggregate reference resolver for the Place aggregate (ADR 0004 §3, ADR 0009).
//!
//! `AssertEnclosedBy` links a place to its enclosing place, so the aggregate-tax check (does the
//! enclosing place exist?) is resolved before `decide` by the `cqrs-es` `Services` slot — symmetric
//! with the Event→Place check. `AssertSuccession` links a place to any number of `from`/`to` places
//! the same way (ADR 0026 §4). The trait lives here so the aggregate's `Services` type can name it;
//! the SQLite implementation lives in `vitni-db`.

use async_trait::async_trait;

use crate::ids::PlaceId;
use crate::place::command::PlaceCommand;

/// The resolved facts `decide` needs about other aggregates for one Place command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct PlaceRefs {
    /// Whether the enclosing `Place` exists in the (possibly-lagging) projection.
    pub enclosing_exists: bool,
    /// The first `AssertSuccession` `from`/`to` place id that does *not* exist in the (possibly
    /// lagging) projection, or `None` if every referenced place exists (ADR 0026 §4, the §9
    /// aggregate-tax check). Unused by every other command.
    pub missing_succession_place: Option<PlaceId>,
}

/// Resolves the cross-aggregate facts for a Place command against the read model.
///
/// Implemented by `vitni-db` over the Place projection; the `cqrs-es` `Services` value for the
/// Place aggregate is an `Arc<dyn PlaceRefResolver>`.
#[async_trait]
pub trait PlaceRefResolver: Send + Sync {
    /// Resolves the [`PlaceRefs`] for `command` (e.g. whether the enclosing place exists).
    async fn resolve(&self, command: &PlaceCommand) -> PlaceRefs;
}
