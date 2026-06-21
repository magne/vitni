//! The Event domain-error taxonomy (data-model §10.1).

use thiserror::Error;

use crate::ids::{AssertionId, EventId, PlaceId};

/// A reason the Event aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EventError {
    /// The command targets an event that does not exist.
    #[error("event {0} does not exist")]
    NotFound(EventId),
    /// `CreateEvent` was issued for an event that already exists.
    #[error("event {0} already exists")]
    AlreadyExists(EventId),
    /// The event was linked to a place the projection does not know — the §9 aggregate-tax check,
    /// validated against the (possibly-lagging) Place projection (ADR 0004 §3).
    #[error("event references unknown place {0}")]
    UnknownPlace(PlaceId),
    /// `RetractAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    /// `SupersedeAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    SupersedesMissingAssertion(AssertionId),
}
