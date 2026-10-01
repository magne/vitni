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
    /// The two notes cannot be merged.
    #[error("notes {surviving} and {merged} cannot be merged: {reason}")]
    MergeConflict {
        /// The intended surviving note.
        surviving: NoteId,
        /// The note that would have been merged in.
        merged: NoteId,
        /// Why the merge was refused.
        reason: String,
    },
    /// A note was distinguished from itself.
    #[error("note {0} cannot be distinguished from itself")]
    DistinctFromItself(NoteId),
    /// The pair already holds a live identity decision — merged or distinguished — on this note
    /// (ADR 0039 §1); undo it before deciding again.
    #[error("notes {note} and {other} already have a live identity decision")]
    IdentityDecided {
        /// The note the decision was asked on.
        note: NoteId,
        /// The other note of the pair.
        other: NoteId,
    },
}
