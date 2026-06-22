//! [`EventView`] — the conclusion-layer read model for an Event (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use crate::address::Address;
use crate::date::GenealogicalDate;
use crate::enums::EventType;
use crate::event::decide::evolve;
use crate::event::state::{EventParticipant, EventState};
use crate::ids::{CitationId, EventId, HumanId, NoteId, PlaceId, TagId};
use crate::text::MediaRef;

/// The current best synthesis of an Event, derived from the event log (data-model §6).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventView {
    state: EventState,
}

impl EventView {
    /// Returns `true` once the event has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The event's id, once created.
    #[must_use]
    pub fn event_id(&self) -> Option<EventId> {
        self.state.event_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// The kind of event.
    #[must_use]
    pub fn event_type(&self) -> Option<&EventType> {
        self.state.event_type.as_ref().map(|t| &t.value)
    }

    /// When the event occurred, if asserted.
    #[must_use]
    pub fn date(&self) -> Option<&GenealogicalDate> {
        self.state.date.as_ref().map(|d| &d.value)
    }

    /// Where the event occurred, if linked.
    #[must_use]
    pub fn place_id(&self) -> Option<PlaceId> {
        self.state.place_id.as_ref().map(|p| p.value)
    }

    /// The event's free-text description, if set.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.state.description.as_ref().map(|d| d.value.as_str())
    }

    /// The event's participants, in assertion order.
    #[must_use]
    pub fn participants(&self) -> Vec<&EventParticipant> {
        self.state.participants.iter().map(|p| &p.value).collect()
    }

    /// All currently-live postal addresses, in assertion order.
    #[must_use]
    pub fn addresses(&self) -> Vec<&Address> {
        self.state.addresses.iter().map(|a| &a.value).collect()
    }

    /// All currently-live citations backing the event's claims, in assertion order.
    #[must_use]
    pub fn citations(&self) -> Vec<CitationId> {
        self.state.citations.iter().map(|c| c.value).collect()
    }

    /// All currently-live attached media, in assertion order.
    #[must_use]
    pub fn media(&self) -> Vec<&MediaRef> {
        self.state.media.iter().map(|m| &m.value).collect()
    }

    /// All currently-live attached notes, in assertion order.
    #[must_use]
    pub fn notes(&self) -> Vec<NoteId> {
        self.state.notes.iter().map(|n| n.value).collect()
    }

    /// All currently-applied tags, in assertion order.
    #[must_use]
    pub fn tags(&self) -> Vec<TagId> {
        self.state.tags.iter().map(|t| t.value).collect()
    }

    /// Whether the event is private (Gramps' universal privacy flag).
    #[must_use]
    pub fn private(&self) -> bool {
        self.state.private
    }
}

impl View<EventState> for EventView {
    fn update(&mut self, event: &EventEnvelope<EventState>) {
        evolve(&mut self.state, &event.payload);
    }
}
