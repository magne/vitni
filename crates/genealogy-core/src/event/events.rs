//! Event events — the past-tense assertions the aggregate produces (data-model §10).
//!
//! The type is named `EventEvent`: the *aggregate* is `Event`, and this is its event-sourcing event
//! (matching `PersonEvent`, `PlaceEvent`, …).

use cqrs_es::DomainEvent;
use serde::{Deserialize, Serialize};

use crate::date::GenealogicalDate;
use crate::enums::EventType;
use crate::ids::{AssertionId, EventId, HumanId, PlaceId};
use crate::provenance::{AssertionMeta, EventContext};

/// A single Event assertion plus its provenance envelope (ADR 0004 §1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventEvent {
    /// Identity of this assertion, so a correction can target it (ADR 0004 §2).
    pub assertion_id: AssertionId,
    /// Who / when / why / how sure / on what evidence (data-model §8).
    pub context: EventContext,
    /// The claim itself.
    #[serde(flatten)]
    pub body: EventEventBody,
}

impl EventEvent {
    /// Stamps `body` with the supplied assertion id and context (ADR 0004 §3).
    #[must_use]
    pub fn new(meta: &AssertionMeta, body: EventEventBody) -> Self {
        Self {
            assertion_id: meta.assertion_id,
            context: meta.context.clone(),
            body,
        }
    }
}

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
}

impl EventEventBody {
    /// The variant name, used as the `cqrs-es` event type (ADR 0004 §4).
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::EventCreated { .. } => "EventCreated",
            Self::EventTypeSet { .. } => "EventTypeSet",
            Self::DateAsserted { .. } => "DateAsserted",
            Self::PlaceLinked { .. } => "PlaceLinked",
        }
    }

    /// The payload schema version of this variant (ADR 0004 §4, ADR 0010).
    ///
    /// Versions are **per-variant**: a variant is bumped only when its own payload changes
    /// (additively), so an unevolved variant keeps `1.0` while `EventCreated` is at `2.0`
    /// since it gained `private`. An upcaster (`event::upcasters`) backfills historical
    /// payloads at read time.
    #[must_use]
    pub fn version(&self) -> &'static str {
        match self {
            Self::EventCreated { .. } => "2.0",
            Self::EventTypeSet { .. } | Self::DateAsserted { .. } | Self::PlaceLinked { .. } => "1.0",
        }
    }
}

impl DomainEvent for EventEvent {
    fn event_type(&self) -> String {
        self.body.type_name().to_owned()
    }

    fn event_version(&self) -> String {
        self.body.version().to_owned()
    }
}
