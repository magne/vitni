//! The cross-aggregate reference resolver for the `ResearchNote` aggregate (ADR 0004 §3, ADR 0028).
//!
//! Mirrors the Event/Citation resolvers: `decide` cannot read the Person/Family/Event/Place
//! projections itself, so the aggregate-tax check (does each named subject exist?) is resolved
//! before `decide` by the `cqrs-es` `Services` slot. The trait lives here so the aggregate's
//! `Services` type can name it; the SQLite/Postgres implementation lives in `vitni-db`, which
//! dispatches on each subject's `SubjectRef` variant to the right projection table.

use std::collections::BTreeSet;

use async_trait::async_trait;

use crate::research_note::command::ResearchNoteCommand;
use crate::research_note::subject::SubjectRef;

/// The resolved facts `decide` needs about the subjects named by one `ResearchNote` command.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ResearchNoteRefs {
    /// Every subject the command names (`CreateResearchNote`'s set, or `AddSubject`'s single
    /// subject — recursing through a `SupersedeAssertion` wrapper) that exists in the
    /// (possibly-lagging) projection. `decide` loops over the command's own subjects and rejects
    /// with `UnknownSubject` on the first one absent from this set — the §9 aggregate-tax check,
    /// applied per subject for a multi-subject note.
    pub existing_subjects: BTreeSet<SubjectRef>,
}

/// Resolves the cross-aggregate facts for a `ResearchNote` command against the read model.
///
/// Implemented by `vitni-db` over the Person/Family/Event/Place projections; the `cqrs-es`
/// `Services` value for the `ResearchNote` aggregate is an `Arc<dyn ResearchNoteRefResolver>`.
#[async_trait]
pub trait ResearchNoteRefResolver: Send + Sync {
    /// Resolves the [`ResearchNoteRefs`] for `command` (e.g. whether the named subject exists).
    async fn resolve(&self, command: &ResearchNoteCommand) -> ResearchNoteRefs;
}
