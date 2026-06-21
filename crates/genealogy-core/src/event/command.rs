//! Event commands — imperative operator intent (data-model §10).

use crate::date::GenealogicalDate;
use crate::enums::EventType;
use crate::ids::{AssertionId, EventId, HumanId, PlaceId};
use crate::provenance::AssertionMeta;

/// Operator intent against an Event aggregate (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum EventCommand {
    /// Create a new event of a given type.
    CreateEvent {
        /// The application-generated id for the new event.
        event_id: EventId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// The kind of event.
        event_type: EventType,
        /// Whether the event is private (Gramps' universal privacy flag).
        private: bool,
    },
    /// Set (or change) the event's type.
    SetEventType {
        /// The target event.
        event_id: EventId,
        /// The new event type.
        event_type: EventType,
    },
    /// Assert when the event occurred.
    AssertDate {
        /// The target event.
        event_id: EventId,
        /// The date the event occurred.
        date: GenealogicalDate,
    },
    /// Link the event to the place it occurred (the cross-aggregate reference).
    LinkPlace {
        /// The target event.
        event_id: EventId,
        /// The place the event occurred.
        place_id: PlaceId,
    },
    /// Retract a prior assertion (non-destructive).
    RetractAssertion {
        /// The target event.
        event_id: EventId,
        /// The assertion to retract.
        target: AssertionId,
    },
    /// Supersede a prior assertion with a replacement command.
    SupersedeAssertion {
        /// The target event.
        event_id: EventId,
        /// The assertion to supersede.
        target: AssertionId,
        /// The command producing the replacement assertion.
        replacement: Box<EventCommand>,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the Event aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: EventCommand,
}
