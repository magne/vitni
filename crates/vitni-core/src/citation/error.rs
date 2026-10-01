//! The Citation domain-error taxonomy (data-model §10.1).

use thiserror::Error;

use crate::ids::{AssertionId, CitationId, SourceId};

/// A reason the Citation aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum CitationError {
    /// The command targets a citation that does not exist.
    #[error("citation {0} does not exist")]
    NotFound(CitationId),
    /// `CreateCitation` was issued for a citation that already exists.
    #[error("citation {0} already exists")]
    AlreadyExists(CitationId),
    /// The citation was created against a source the projection does not know — the §9
    /// aggregate-tax check, validated against the (possibly-lagging) Source projection
    /// (ADR 0004 §3).
    #[error("citation references unknown source {0}")]
    UnknownSource(SourceId),
    /// `RetractAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    /// `SupersedeAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    SupersedesMissingAssertion(AssertionId),
    /// The two citations cannot be merged.
    #[error("citations {surviving} and {merged} cannot be merged: {reason}")]
    MergeConflict {
        /// The intended surviving citation.
        surviving: CitationId,
        /// The citation that would have been merged in.
        merged: CitationId,
        /// Why the merge was refused.
        reason: String,
    },
    /// A citation was distinguished from itself.
    #[error("citation {0} cannot be distinguished from itself")]
    DistinctFromItself(CitationId),
    /// The pair already holds a live identity decision — merged or distinguished — on this citation
    /// (ADR 0039 §1); undo it before deciding again.
    #[error("citations {citation} and {other} already have a live identity decision")]
    IdentityDecided {
        /// The citation the decision was asked on.
        citation: CitationId,
        /// The other citation of the pair.
        other: CitationId,
    },
}
