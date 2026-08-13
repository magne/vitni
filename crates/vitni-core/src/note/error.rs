//! The Note domain-error taxonomy (data-model §10.1).

use thiserror::Error;

use crate::ids::{AssertionId, NoteId};

/// A reason the Note aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum NoteError {
    /// The command targets a note that does not exist.
    #[error("note {0} does not exist")]
    NotFound(NoteId),
    /// `CreateNote` was issued for a note that already exists.
    #[error("note {0} already exists")]
    AlreadyExists(NoteId),
    /// `RetractAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    /// `SupersedeAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    SupersedesMissingAssertion(AssertionId),
}
