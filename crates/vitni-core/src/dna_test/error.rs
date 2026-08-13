//! The `DnaTest` domain-error taxonomy (data-model §10.1, §12).

use thiserror::Error;

use crate::ids::{AssertionId, DnaTestId, PersonId};

/// A reason the `DnaTest` aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum DnaTestError {
    /// The command targets a test that does not exist.
    #[error("dna test {0} does not exist")]
    NotFound(DnaTestId),
    /// `CreateDnaTest` was issued for a test that already exists.
    #[error("dna test {0} already exists")]
    AlreadyExists(DnaTestId),
    /// `CreateDnaTest` anchored the test to a person the projection does not know (the §9
    /// aggregate-tax check).
    #[error("person {0} does not exist")]
    UnknownPerson(PersonId),
    /// `RetractAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    /// `SupersedeAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    SupersedesMissingAssertion(AssertionId),
}
