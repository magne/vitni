//! The Source domain-error taxonomy (data-model §10.1).

use thiserror::Error;

use crate::ids::{AssertionId, RepositoryId, SourceId};

/// A reason the Source aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SourceError {
    /// The command targets a source that does not exist.
    #[error("source {0} does not exist")]
    NotFound(SourceId),
    /// `CreateSource` was issued for a source that already exists.
    #[error("source {0} already exists")]
    AlreadyExists(SourceId),
    /// `LinkRepository` referenced a repository the projection does not know (the §9 aggregate-tax
    /// check).
    #[error("repository {0} does not exist")]
    UnknownRepository(RepositoryId),
    /// `RetractAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    /// `SupersedeAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    SupersedesMissingAssertion(AssertionId),
    /// The two sources cannot be merged.
    #[error("sources {surviving} and {merged} cannot be merged: {reason}")]
    MergeConflict {
        /// The intended surviving source.
        surviving: SourceId,
        /// The source that would have been merged in.
        merged: SourceId,
        /// Why the merge was refused.
        reason: String,
    },
    /// A source was distinguished from itself.
    #[error("source {0} cannot be distinguished from itself")]
    DistinctFromItself(SourceId),
    /// The pair already holds a live identity decision — merged or distinguished — on this source
    /// (ADR 0039 §1); undo it before deciding again.
    #[error("sources {source_id} and {other} already have a live identity decision")]
    IdentityDecided {
        /// The source the decision was asked on (not `source`, which `thiserror` reads as the cause).
        source_id: SourceId,
        /// The other source of the pair.
        other: SourceId,
    },
}
