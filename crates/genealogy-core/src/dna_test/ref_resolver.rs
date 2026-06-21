//! The cross-aggregate reference resolver for the `DnaTest` aggregate (ADR 0004 §3, ADR 0009).
//!
//! A test is anchored to one Person (data-model §12), so the aggregate-tax check (does the person
//! exist?) is resolved before `decide` by the `cqrs-es` `Services` slot. The trait lives here so the
//! aggregate's `Services` type can name it; the SQLite implementation lives in `genealogy-db`.

use async_trait::async_trait;

use crate::dna_test::command::DnaTestCommand;

/// The resolved facts `decide` needs about other aggregates for one `DnaTest` command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct DnaTestRefs {
    /// Whether the anchoring `Person` exists in the (possibly-lagging) projection.
    pub person_exists: bool,
}

/// Resolves the cross-aggregate facts for a `DnaTest` command against the read model.
///
/// Implemented by `genealogy-db` over the Person projection; the `cqrs-es` `Services` value for the
/// `DnaTest` aggregate is an `Arc<dyn DnaTestRefResolver>`.
#[async_trait]
pub trait DnaTestRefResolver: Send + Sync {
    /// Resolves the [`DnaTestRefs`] for `command` (e.g. whether the anchoring person exists).
    async fn resolve(&self, command: &DnaTestCommand) -> DnaTestRefs;
}
