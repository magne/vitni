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
    /// The two media objects cannot be merged.
    #[error("media objects {surviving} and {merged} cannot be merged: {reason}")]
    MergeConflict {
        /// The intended surviving media object.
        surviving: MediaId,
        /// The media object that would have been merged in.
        merged: MediaId,
        /// Why the merge was refused.
        reason: String,
    },
    /// A media object was distinguished from itself.
    #[error("media object {0} cannot be distinguished from itself")]
    DistinctFromItself(MediaId),
    /// The pair already holds a live identity decision — merged or distinguished — on this media object
    /// (ADR 0039 §1); undo it before deciding again.
    #[error("media objects {media} and {other} already have a live identity decision")]
    IdentityDecided {
        /// The media object the decision was asked on.
        media: MediaId,
        /// The other media object of the pair.
        other: MediaId,
    },
}
