//! The Family domain-error taxonomy (data-model §10.1).
//!
//! These are the rejection half of `decide -> Result<Vec<FamilyEvent>, FamilyError>`: domain
//! rejections, never infrastructure failures. They become the `cqrs-es` `Aggregate::Error`.

use thiserror::Error;

use crate::ids::{AssertionId, FamilyId, PersonId};

/// A reason the Family aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum FamilyError {
    /// The command targets a family that does not exist.
    #[error("family {0} does not exist")]
    NotFound(FamilyId),
    /// `CreateFamily` was issued for a family that already exists.
    #[error("family {0} already exists")]
    AlreadyExists(FamilyId),
    /// A partner was added who is already a partner of the family.
    #[error("person {0} is already a partner of this family")]
    PartnerAlreadyPresent(PersonId),
    /// A partner was removed who is not a partner of the family.
    #[error("person {0} is not a partner of this family")]
    PartnerNotPresent(PersonId),
    /// A child was added who is already a child of the family.
    #[error("person {0} is already a child of this family")]
    ChildAlreadyPresent(PersonId),
    /// A child was removed who is not a child of the family.
    #[error("person {0} is not a child of this family")]
    ChildNotPresent(PersonId),
    /// A child–parent relationship named a parent who is not a current partner of the family.
    #[error("person {0} is not a partner of this family")]
    ParentNotPartner(PersonId),
    /// A child–parent relationship was asserted for a `(child, parent)` pair already present.
    #[error("child {0} already has a relationship to partner {1}")]
    ChildRelationshipAlreadyPresent(PersonId, PersonId),
    /// The retracted assertion is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    /// The superseded assertion is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    SupersedesMissingAssertion(AssertionId),
}
