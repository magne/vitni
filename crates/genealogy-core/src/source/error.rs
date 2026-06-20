//! The Source domain-error taxonomy (data-model §10.1).

use thiserror::Error;

use crate::ids::{AssertionId, SourceId};

/// A reason the Source aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SourceError {
    /// The command targets a source that does not exist.
    #[error("source {0} does not exist")]
    NotFound(SourceId),
    /// `CreateSource` was issued for a source that already exists.
    #[error("source {0} already exists")]
    AlreadyExists(SourceId),
    /// `RetractAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    /// `SupersedeAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    SupersedesMissingAssertion(AssertionId),
}
