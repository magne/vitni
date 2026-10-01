//! The Repository domain-error taxonomy (data-model §10.1).

use thiserror::Error;

use crate::ids::{AssertionId, RepositoryId};

/// A reason the Repository aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum RepositoryError {
    /// The command targets a repository that does not exist.
    #[error("repository {0} does not exist")]
    NotFound(RepositoryId),
    /// `CreateRepository` was issued for a repository that already exists.
    #[error("repository {0} already exists")]
    AlreadyExists(RepositoryId),
    /// A name was set with no text.
    #[error("a repository name must not be empty")]
    EmptyName,
    /// `RetractAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    /// `SupersedeAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    SupersedesMissingAssertion(AssertionId),
    /// The two repositories cannot be merged.
    #[error("repositories {surviving} and {merged} cannot be merged: {reason}")]
    MergeConflict {
        /// The intended surviving repository.
        surviving: RepositoryId,
        /// The repository that would have been merged in.
        merged: RepositoryId,
        /// Why the merge was refused.
        reason: String,
    },
    /// A repository was distinguished from itself.
    #[error("repository {0} cannot be distinguished from itself")]
    DistinctFromItself(RepositoryId),
    /// The pair already holds a live identity decision — merged or distinguished — on this repository
    /// (ADR 0039 §1); undo it before deciding again.
    #[error("repositories {repository} and {other} already have a live identity decision")]
    IdentityDecided {
        /// The repository the decision was asked on.
        repository: RepositoryId,
        /// The other repository of the pair.
        other: RepositoryId,
    },
}
