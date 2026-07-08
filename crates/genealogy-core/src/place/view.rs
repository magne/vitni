//! [`PlaceView`] — the conclusion-layer read model for a Place (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (it delegates to `evolve`). The denormalized
//! SQL read schema is deferred (ADR 0009); the view exposes its projected fields through accessors
//! over the folded state.

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::{Asserted, Attributed};
use crate::enums::{PlaceType, Restriction};
use crate::geo::GeoCoordinates;
use crate::ids::{CitationId, HumanId, NoteId, PlaceId, TagId};
use crate::place::decide::evolve;
use crate::place::state::PlaceState;
use crate::place_name::PlaceName;
use crate::place_ref::PlaceRef;
use crate::text::MediaRef;

/// The current best synthesis of a Place, derived from the event log (data-model §6).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceView {
    state: PlaceState,
}

impl PlaceView {
    /// Returns `true` once the place has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The place's id, once created.
    #[must_use]
    pub fn place_id(&self) -> Option<PlaceId> {
        self.state.place_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// The place's type.
    #[must_use]
    pub fn place_type(&self) -> Option<&PlaceType> {
        self.state.place_type.as_ref().map(|t| &t.value.value)
    }

    /// The place's type with its provenance (surety + backing citations), if set.
    #[must_use]
    pub fn asserted_place_type(&self) -> Option<&Asserted<PlaceType>> {
        self.state.place_type.as_ref().map(|t| &t.value)
    }

    /// All currently-live asserted names, in assertion order.
    #[must_use]
    pub fn names(&self) -> Vec<&PlaceName> {
        self.state.names.iter().map(|n| &n.value.value).collect()
    }

    /// All currently-live asserted names with their provenance, in assertion order.
    #[must_use]
    pub fn asserted_names(&self) -> Vec<&Asserted<PlaceName>> {
        self.state.names.iter().map(|n| &n.value).collect()
    }

    /// All currently-live enclosing-place relationships, in assertion order.
    #[must_use]
    pub fn enclosed_by(&self) -> Vec<&PlaceRef> {
        self.state.enclosed_by.iter().map(|e| &e.value.value).collect()
    }

    /// All currently-live enclosing-place relationships with their provenance, in assertion order.
    #[must_use]
    pub fn asserted_enclosed_by(&self) -> Vec<&Asserted<PlaceRef>> {
        self.state.enclosed_by.iter().map(|e| &e.value).collect()
    }

    /// The place's coordinates, if asserted.
    #[must_use]
    pub fn coordinates(&self) -> Option<&GeoCoordinates> {
        self.state.coordinates.as_ref().map(|c| &c.value.value)
    }

    /// The place's coordinates with their provenance (surety + backing citations), if asserted.
    #[must_use]
    pub fn asserted_coordinates(&self) -> Option<&Asserted<GeoCoordinates>> {
        self.state.coordinates.as_ref().map(|c| &c.value)
    }

    /// The place's code, if set.
    #[must_use]
    pub fn code(&self) -> Option<&str> {
        self.state.code.as_ref().map(|c| c.value.value.as_str())
    }

    /// The place's code with its provenance (surety + backing citations), if set.
    #[must_use]
    pub fn asserted_code(&self) -> Option<&Asserted<String>> {
        self.state.code.as_ref().map(|c| &c.value)
    }

    /// All currently-live citations backing the place's claims, in assertion order.
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

    /// The place's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }

    /// Currently-live names, each paired with the `AssertionId` that introduced it — the read side of
    /// the per-row correction (Edit supersedes it, Remove retracts it).
    #[must_use]
    pub fn names_with_assertions(&self) -> &[Attributed<Asserted<PlaceName>>] {
        &self.state.names
    }

    /// Currently-live enclosing-place relationships, each paired with its introducing `AssertionId`.
    #[must_use]
    pub fn enclosed_by_with_assertions(&self) -> &[Attributed<Asserted<PlaceRef>>] {
        &self.state.enclosed_by
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

impl View<PlaceState> for PlaceView {
    fn update(&mut self, event: &EventEnvelope<PlaceState>) {
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
        let state = PlaceState {
            notes: vec![Attributed {
                assertion_id: aid,
                value: note,
            }],
            ..Default::default()
        };
        let view = PlaceView { state };
        assert_eq!(view.notes_with_assertions()[0].assertion_id, aid);
    }
}
