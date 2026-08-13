//! Event commands — imperative operator intent (data-model §10).

use std::collections::BTreeSet;

use crate::address::Address;
use crate::date::GenealogicalDate;
use crate::enums::{EventType, Restriction};
use crate::ids::{AssertionId, CitationId, EventId, HumanId, NoteId, PlaceId, TagId};
use crate::provenance::AssertionMeta;
use crate::text::MediaRef;

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
    /// Set (or change) the event's free-text description.
    SetDescription {
        /// The target event.
        event_id: EventId,
        /// The description.
        description: String,
    },
    /// Link the event to the place it occurred (the cross-aggregate reference).
    LinkPlace {
        /// The target event.
        event_id: EventId,
        /// The place the event occurred.
        place_id: PlaceId,
    },
    /// Add a postal address to the event (e.g. a residence or census address — data-model §7, §17).
    AddAddress {
        /// The target event.
        event_id: EventId,
        /// The address.
        address: Address,
    },
    /// Add a citation backing the event's claims.
    AddCitation {
        /// The target event.
        event_id: EventId,
        /// The citation to add.
        citation_id: CitationId,
    },
    /// Attach a media reference to the event.
    AttachMedia {
        /// The target event.
        event_id: EventId,
        /// The media reference.
        media: MediaRef,
    },
    /// Attach a note to the event.
    AttachNote {
        /// The target event.
        event_id: EventId,
        /// The note to attach.
        note_id: NoteId,
    },
    /// Apply a tag to the event.
    Tag {
        /// The target event.
        event_id: EventId,
        /// The tag to apply.
        tag_id: TagId,
    },
    /// Remove a tag from the event.
    Untag {
        /// The target event.
        event_id: EventId,
        /// The tag to remove.
        tag_id: TagId,
    },
    /// Set (or change) the event's privacy restrictions (GEDCOM `RESN` — data-model §6).
    SetRestrictions {
        /// The target event.
        event_id: EventId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
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
    /// Set (or change) the event's user-facing identifier (data-model §7).
    SetHumanId {
        /// The target event.
        event_id: EventId,
        /// The new user-facing identifier.
        human_id: HumanId,
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
