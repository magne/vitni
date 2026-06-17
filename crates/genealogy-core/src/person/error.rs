//! The Person domain-error taxonomy (data-model §10.1).
//!
//! These are the rejection half of `decide -> Result<Vec<PersonEvent>, PersonError>`: domain
//! rejections, never infrastructure failures. They become the `cqrs-es` `Aggregate::Error`.

use thiserror::Error;

use crate::ids::{AssertionId, PersonId};

/// A reason the Person aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PersonError {
    /// The command targets a person that does not exist.
    #[error("person {0} does not exist")]
    NotFound(PersonId),
    /// `CreatePerson` was issued for a person that already exists.
    #[error("person {0} already exists")]
    AlreadyExists(PersonId),
    /// A name was asserted with neither a given name nor a surname.
    #[error("name must have a given name or a surname")]
    EmptyName,
    /// The retracted assertion is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    /// The superseded assertion is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    SupersedesMissingAssertion(AssertionId),
    /// A date that cannot be ordered or is internally inconsistent.
    #[error("invalid date: {0}")]
    InvalidDate(String),
    /// The two persons cannot be merged (e.g. contradicting irreversible facts).
    #[error("persons {surviving} and {merged} cannot be merged: {reason}")]
    MergeConflict {
        /// The intended surviving person.
        surviving: PersonId,
        /// The person that would have been merged in.
        merged: PersonId,
        /// Why the merge was refused.
        reason: String,
    },
    /// A person was associated with itself.
    #[error("person {0} cannot be associated with itself")]
    SelfAssociation(PersonId),
}
