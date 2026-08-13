//! The Media domain-error taxonomy (data-model §10.1).

use thiserror::Error;

use crate::ids::{AssertionId, MediaId};

/// A reason the Media aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum MediaError {
    /// The command targets a media object that does not exist.
    #[error("media {0} does not exist")]
    NotFound(MediaId),
    /// `CreateMedia` was issued for a media object that already exists.
    #[error("media {0} already exists")]
    AlreadyExists(MediaId),
    /// `RetractAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    /// `SupersedeAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    SupersedesMissingAssertion(AssertionId),
}
