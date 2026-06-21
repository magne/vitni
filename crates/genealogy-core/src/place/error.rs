//! The Place domain-error taxonomy (data-model §10.1).
//!
//! Domain rejections (the rejection half of `decide`), never infrastructure failures. They become
//! the `cqrs-es` `Aggregate::Error`.

use thiserror::Error;

use crate::ids::{AssertionId, PlaceId};

/// A reason the Place aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlaceError {
    /// The command targets a place that does not exist.
    #[error("place {0} does not exist")]
    NotFound(PlaceId),
    /// `CreatePlace` was issued for a place that already exists.
    #[error("place {0} already exists")]
    AlreadyExists(PlaceId),
    /// A name was asserted with no text.
    #[error("a place name must not be empty")]
    EmptyName,
    /// A code was set with no text.
    #[error("a place code must not be empty")]
    EmptyCode,
    /// `AssertEnclosedBy` referenced an enclosing place the projection does not know (the §9
    /// aggregate-tax check).
    #[error("enclosing place {0} does not exist")]
    UnknownPlace(PlaceId),
    /// `RetractAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    /// `SupersedeAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    SupersedesMissingAssertion(AssertionId),
}
