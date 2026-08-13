//! The `DnaMatch` domain-error taxonomy (data-model §10.1, §12).

use thiserror::Error;

use crate::ids::{AssertionId, DnaMatchId, DnaTestId};

/// A reason the `DnaMatch` aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DnaMatchError {
    /// The command targets a match that does not exist.
    #[error("dna match {0} does not exist")]
    NotFound(DnaMatchId),
    /// `ObserveMatch` was issued for a match that already exists.
    #[error("dna match {0} already exists")]
    AlreadyExists(DnaMatchId),
    /// `ObserveMatch` referenced a test the projection does not know (the §9 aggregate-tax check).
    #[error("dna test {0} does not exist")]
    UnknownTest(DnaTestId),
    /// `ObserveMatch` had the same test on both sides.
    #[error("a match cannot be between a test ({0}) and itself")]
    SameTestBothSides(DnaTestId),
    /// `ObserveMatch` had a negative shared-cM total.
    #[error("shared centimorgans must not be negative")]
    NegativeSharedCm,
    /// `RetractAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    /// `SupersedeAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    SupersedesMissingAssertion(AssertionId),
}
