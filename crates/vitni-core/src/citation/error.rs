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
}
