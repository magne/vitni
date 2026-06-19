//! The cross-aggregate reference resolver for the Citation aggregate (ADR 0004 §3, ADR 0009).
//!
//! `decide` is pure and cannot read another aggregate's projection. The aggregate-tax check
//! (data-model §9) — does the cited `Source` exist? — is therefore resolved *before* `decide` by
//! the `cqrs-es` `Services` slot, which ADR 0004 §3 reserves for exactly these cross-aggregate
//! projection reads. The adapter ([`crate::citation::aggregate`]) calls [`CitationRefResolver`]
//! and passes the resolved [`CitationRefs`] into `decide`, keeping the rule in the pure core while
//! the impure read stays at the edge.
//!
//! The trait lives in `genealogy-core` so the aggregate's `Services` type can name it; the concrete
//! implementation (a SQLite projection query) lives in `genealogy-db`.

use async_trait::async_trait;

use crate::citation::command::CitationCommand;

/// The resolved facts `decide` needs about other aggregates for one Citation command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CitationRefs {
    /// Whether the cited `Source` exists in the (possibly-lagging) projection.
    pub source_exists: bool,
}

/// Resolves the cross-aggregate facts for a Citation command against the read model.
///
/// Implemented by `genealogy-db` over the Source projection; the `cqrs-es` `Services` value for the
/// Citation aggregate is an `Arc<dyn CitationRefResolver>`.
#[async_trait]
pub trait CitationRefResolver: Send + Sync {
    /// Resolves the [`CitationRefs`] for `command` (e.g. whether the cited source exists).
    async fn resolve(&self, command: &CitationCommand) -> CitationRefs;
}
