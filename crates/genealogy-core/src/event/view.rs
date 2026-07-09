//! [`EventView`] — the conclusion-layer read model for an Event (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use std::collections::BTreeSet;

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use crate::address::Address;
use crate::assertions::{Asserted, Attributed};
use crate::date::GenealogicalDate;
use crate::enums::{EventType, Restriction};
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
        self.state.event_type.as_ref().map(|t| &t.value.value)
    }

    /// The kind of event with its provenance (surety + backing citations), if asserted.
    #[must_use]
    pub fn asserted_event_type(&self) -> Option<&Asserted<EventType>> {
        self.state.event_type.as_ref().map(|t| &t.value)
    }

    /// When the event occurred, if asserted.
    #[must_use]
    pub fn date(&self) -> Option<&GenealogicalDate> {
        self.state.date.as_ref().map(|d| &d.value.value)
    }

    /// When the event occurred with its provenance (surety + backing citations), if asserted.
    #[must_use]
    pub fn asserted_date(&self) -> Option<&Asserted<GenealogicalDate>> {
        self.state.date.as_ref().map(|d| &d.value)
    }

    /// Where the event occurred, if linked.
    #[must_use]
    pub fn place_id(&self) -> Option<PlaceId> {
        self.state.place_id.as_ref().map(|p| p.value.value)
    }

    /// Where the event occurred with its provenance (surety + backing citations), if linked.
    #[must_use]
    pub fn asserted_place(&self) -> Option<&Asserted<PlaceId>> {
        self.state.place_id.as_ref().map(|p| &p.value)
    }

    /// The event's free-text description, if set.
    #[must_use]
    pub fn description(&self) -> Option<&str> {
        self.state.description.as_ref().map(|d| d.value.value.as_str())
    }

    /// The event's free-text description with its provenance, if set.
    #[must_use]
    pub fn asserted_description(&self) -> Option<&Asserted<String>> {
        self.state.description.as_ref().map(|d| &d.value)
    }

    /// The event's participants, in assertion order.
    #[must_use]
    pub fn participants(&self) -> Vec<&EventParticipant> {
        self.state.participants.iter().map(|p| &p.value.value).collect()
    }

    /// The event's participants with their provenance (surety + backing citations), in assertion order.
    #[must_use]
    pub fn asserted_participants(&self) -> Vec<&Asserted<EventParticipant>> {
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

    /// The event's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }

    /// Currently-live participants, each paired with the `AssertionId` that introduced it — the read
    /// side of the per-row correction (Edit supersedes it, Remove retracts it).
    #[must_use]
    pub fn participants_with_assertions(&self) -> &[Attributed<Asserted<EventParticipant>>] {
        &self.state.participants
    }

    /// Currently-live citations, each paired with its introducing `AssertionId`.
    #[must_use]
    pub fn citations_with_assertions(&self) -> &[Attributed<CitationId>] {
        &self.state.citations
    }

    /// Currently-live attached media, each paired with the attach `AssertionId` (the detach target).
    #[must_use]
    pub fn media_with_assertions(&self) -> &[Attributed<MediaRef>] {
        &self.state.media
    }

    /// Currently-live attached notes, each paired with the attach `AssertionId` (the detach target).
    #[must_use]
    pub fn notes_with_assertions(&self) -> &[Attributed<NoteId>] {
        &self.state.notes
    }
}

impl View<EventState> for EventView {
    fn update(&mut self, event: &EventEnvelope<EventState>) {
        evolve(&mut self.state, &event.payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assertions::Attributed;
    use crate::ids::AssertionId;
    use uuid::Uuid;

    #[test]
    fn notes_with_assertions_exposes_the_attach_assertion() {
        let aid = AssertionId::from_uuid(Uuid::from_u128(7));
        let note = crate::ids::NoteId::from_uuid(Uuid::from_u128(8));
        let state = EventState {
            notes: vec![Attributed {
                assertion_id: aid,
                value: note,
            }],
            ..Default::default()
        };
        let view = EventView { state };
        assert_eq!(view.notes_with_assertions()[0].assertion_id, aid);
    }
}
