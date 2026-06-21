//! The cross-aggregate reference resolver for the `DnaMatch` aggregate (ADR 0004 §3, ADR 0009).
//!
//! A match is a pairwise observation between two `DnaTest`s (data-model §12), so the aggregate-tax
//! check (do both tests exist?) is resolved before `decide` by the `cqrs-es` `Services` slot. The
//! trait lives here so the aggregate's `Services` type can name it; the SQLite implementation lives
//! in `genealogy-db`.

use async_trait::async_trait;

use crate::dna_match::command::DnaMatchCommand;

/// The resolved facts `decide` needs about other aggregates for one `DnaMatch` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DnaMatchRefs {
    /// Whether `test_a` exists in the (possibly-lagging) projection.
    pub test_a_exists: bool,
    /// Whether `test_b` exists in the (possibly-lagging) projection.
    pub test_b_exists: bool,
}

/// Resolves the cross-aggregate facts for a `DnaMatch` command against the read model.
///
/// Implemented by `genealogy-db` over the `DnaTest` projection; the `cqrs-es` `Services` value for
/// the `DnaMatch` aggregate is an `Arc<dyn DnaMatchRefResolver>`.
#[async_trait]
pub trait DnaMatchRefResolver: Send + Sync {
    /// Resolves the [`DnaMatchRefs`] for `command` (e.g. whether both tests exist).
    async fn resolve(&self, command: &DnaMatchCommand) -> DnaMatchRefs;
}
