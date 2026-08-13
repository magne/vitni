//! The cross-aggregate reference resolver for the Source aggregate (ADR 0004 §3, ADR 0009).
//!
//! `LinkRepository` links a source to a repository that holds it, so the aggregate-tax check (does
//! the repository exist?) is resolved before `decide` by the `cqrs-es` `Services` slot — mirroring
//! Citation→Source. The trait lives here so the aggregate's `Services` type can name it; the SQLite
//! implementation lives in `vitni-db`.

use async_trait::async_trait;

use crate::source::command::SourceCommand;

/// The resolved facts `decide` needs about other aggregates for one Source command.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SourceRefs {
    /// Whether the linked `Repository` exists in the (possibly-lagging) projection.
    pub repository_exists: bool,
}

/// Resolves the cross-aggregate facts for a Source command against the read model.
///
/// Implemented by `vitni-db` over the Repository projection; the `cqrs-es` `Services` value for
/// the Source aggregate is an `Arc<dyn SourceRefResolver>`.
#[async_trait]
pub trait SourceRefResolver: Send + Sync {
    /// Resolves the [`SourceRefs`] for `command` (e.g. whether the linked repository exists).
    async fn resolve(&self, command: &SourceCommand) -> SourceRefs;
}
