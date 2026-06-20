//! Event events — the past-tense assertions the aggregate produces (data-model §10).
//!
//! The type is named `EventEvent`: the *aggregate* is `Event`, and this is its event-sourcing event
//! (matching `PersonEvent`, `PlaceEvent`, …).

use serde::{Deserialize, Serialize};

use crate::assertions::{Envelope, EventBody};
use crate::date::GenealogicalDate;
use crate::enums::EventType;
use crate::ids::{AssertionId, EventId, HumanId, PlaceId};

/// A single Event assertion plus its provenance envelope (ADR 0004 §1).
pub type EventEvent = Envelope<EventEventBody>;

/// The Event claim variants (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum EventEventBody {
    /// An event aggregate was created.
    EventCreated {
        /// The created event.
        event_id: EventId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// The kind of event.
        event_type: EventType,
        /// Whether the event is private (Gramps' universal privacy flag). Added in payload
        /// version `2.0`; historical `1.0` events are upcast to `false` (ADR 0010).
        private: bool,
    },
    /// The event's type was set / changed.
    EventTypeSet {
        /// The event.
        event_id: EventId,
        /// The new event type.
        event_type: EventType,
    },
    /// The event's date was asserted.
    DateAsserted {
        /// The event.
        event_id: EventId,
        /// The asserted date.
        date: GenealogicalDate,
    },
    /// The event was linked to the place it occurred.
    PlaceLinked {
        /// The event.
        event_id: EventId,
        /// The place the event occurred.
        place_id: PlaceId,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The event.
        event_id: EventId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The event.
        event_id: EventId,
        /// The assertion being superseded.
        target: AssertionId,
    },
}

impl EventBody for EventEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::EventCreated { .. } => "EventCreated",
            Self::EventTypeSet { .. } => "EventTypeSet",
            Self::DateAsserted { .. } => "DateAsserted",
            Self::PlaceLinked { .. } => "PlaceLinked",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
        }
    }

    /// Versions are **per-variant**: a variant is bumped only when its own payload changes
    /// (additively), so an unevolved variant keeps `1.0` while `EventCreated` is at `2.0`
    /// since it gained `private`. An upcaster (`event::upcasters`) backfills historical
    /// payloads at read time.
    fn version(&self) -> &'static str {
        match self {
            Self::EventCreated { .. } => "2.0",
            Self::EventTypeSet { .. }
            | Self::DateAsserted { .. }
            | Self::PlaceLinked { .. }
            | Self::AssertionRetracted { .. }
            | Self::AssertionSuperseded { .. } => "1.0",
        }
    }
}
