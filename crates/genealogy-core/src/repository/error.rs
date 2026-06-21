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
}
