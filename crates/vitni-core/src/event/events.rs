//! Event events — the past-tense assertions the aggregate produces (data-model §10).
//!
//! The type is named `EventEvent`: the *aggregate* is `Event`, and this is its event-sourcing event
//! (matching `PersonEvent`, `PlaceEvent`, …).

use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::address::Address;
use crate::assertions::{Envelope, EventBody};
use crate::date::GenealogicalDate;
use crate::enums::{EventType, Restriction};
use crate::ids::{AssertionId, CitationId, EventId, HumanId, NoteId, PlaceId, TagId};
use crate::text::MediaRef;

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
    /// The event's free-text description was set / changed.
    DescriptionSet {
        /// The event.
        event_id: EventId,
        /// The description.
        description: String,
    },
    /// The event was linked to the place it occurred.
    PlaceLinked {
        /// The event.
        event_id: EventId,
        /// The place the event occurred.
        place_id: PlaceId,
    },
    /// A postal address was added to the event (data-model §7, §17).
    AddressAdded {
        /// The event.
        event_id: EventId,
        /// The address.
        address: Address,
    },
    /// A citation was added to the event.
    CitationAdded {
        /// The event.
        event_id: EventId,
        /// The added citation.
        citation_id: CitationId,
    },
    /// A media reference was attached to the event.
    MediaAttached {
        /// The event.
        event_id: EventId,
        /// The media reference.
        media: MediaRef,
    },
    /// A note was attached to the event.
    NoteAttached {
        /// The event.
        event_id: EventId,
        /// The attached note.
        note_id: NoteId,
    },
    /// A tag was applied to the event.
    Tagged {
        /// The event.
        event_id: EventId,
        /// The applied tag.
        tag_id: TagId,
    },
    /// A tag was removed from the event.
    Untagged {
        /// The event.
        event_id: EventId,
        /// The removed tag.
        tag_id: TagId,
    },
    /// The event's privacy restrictions were set / changed (GEDCOM `RESN` — data-model §6).
    RestrictionsChanged {
        /// The event.
        event_id: EventId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
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
    /// The event's user-facing identifier was changed (data-model §7).
    HumanIdChanged {
        /// The event.
        event_id: EventId,
        /// The new user-facing identifier.
        human_id: HumanId,
        /// The identifier in effect before this change (for the audit trail).
        old_human_id: HumanId,
    },
}

impl EventBody for EventEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::EventCreated { .. } => "EventCreated",
            Self::EventTypeSet { .. } => "EventTypeSet",
            Self::DateAsserted { .. } => "DateAsserted",
            Self::DescriptionSet { .. } => "DescriptionSet",
            Self::PlaceLinked { .. } => "PlaceLinked",
            Self::AddressAdded { .. } => "AddressAdded",
            Self::CitationAdded { .. } => "CitationAdded",
            Self::MediaAttached { .. } => "MediaAttached",
            Self::NoteAttached { .. } => "NoteAttached",
            Self::Tagged { .. } => "Tagged",
            Self::Untagged { .. } => "Untagged",
            Self::RestrictionsChanged { .. } => "RestrictionsChanged",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
            Self::HumanIdChanged { .. } => "HumanIdChanged",
        }
    }

    fn version(&self) -> &'static str {
        "1.0"
    }
}
