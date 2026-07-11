//! [`FamilyView`] — the conclusion-layer read model for a Family (data-model §6).
//!
//! The view is rebuilt by folding the same events as the aggregate (it delegates to `evolve`), so
//! corrections — retractions and supersessions — are reflected correctly. A denormalized SQL read
//! schema is deferred (ADR 0002, data-model §17); for now the view exposes its projected fields
//! through accessor methods over the folded state.

use std::collections::BTreeSet;

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use crate::assertions::{Asserted, Attributed};
use crate::enums::Restriction;
use crate::family::decide::evolve;
use crate::family::state::{
    AssertedChild, AssertedFamilyEvent, AssertedPartner, ChildEntry, ChildRelationship, FamilyState,
};
use crate::ids::{CitationId, EventId, FamilyId, HumanId, NoteId, PersonId, TagId};
use crate::text::{ExternalId, MediaRef};

/// The current best synthesis of a Family, derived from the event log (data-model §6).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyView {
    state: FamilyState,
}

impl FamilyView {
    /// Returns `true` once the family has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The family's id, once created.
    #[must_use]
    pub fn family_id(&self) -> Option<FamilyId> {
        self.state.family_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// All currently-live partner participations (retracted ones are excluded).
    #[must_use]
    pub fn partners(&self) -> Vec<PersonId> {
        self.state.partners.iter().map(|p| p.value.person_id).collect()
    }

    /// All currently-live partners with their provenance (surety + backing citations).
    #[must_use]
    pub fn asserted_partners(&self) -> Vec<&AssertedPartner> {
        self.state.partners.iter().map(|p| &p.value).collect()
    }

    /// All currently-live children (retracted ones are excluded), each reconstructed with its
    /// per-partner relationships folded from the [`ChildRelationship`] rows (ADR 0021).
    #[must_use]
    pub fn children(&self) -> Vec<ChildEntry> {
        self.state
            .children
            .iter()
            .map(|c| {
                let child_id = c.value.child_id;
                let relationships = self
                    .state
                    .child_relationships
                    .iter()
                    .filter(|r| r.value.value.child_id == child_id)
                    .map(|r| (r.value.value.parent_id, r.value.value.relationship.clone()))
                    .collect();
                ChildEntry {
                    child_id,
                    relationships,
                }
            })
            .collect()
    }

    /// All currently-live children (membership) with their provenance (surety + backing citations).
    #[must_use]
    pub fn asserted_children(&self) -> Vec<&AssertedChild> {
        self.state.children.iter().map(|c| &c.value).collect()
    }

    /// All currently-live citations backing the family's claims, in assertion order.
    #[must_use]
    pub fn citations(&self) -> Vec<CitationId> {
        self.state.citations.iter().map(|c| c.value).collect()
    }

    /// All currently-live linked family events (e.g. a marriage), in assertion order.
    #[must_use]
    pub fn linked_events(&self) -> Vec<EventId> {
        self.state.linked_events.iter().map(|e| e.value.event_id).collect()
    }

    /// All currently-live linked family events with their provenance (surety + backing citations).
    #[must_use]
    pub fn asserted_linked_events(&self) -> Vec<&AssertedFamilyEvent> {
        self.state.linked_events.iter().map(|e| &e.value).collect()
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

    /// The family's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }

    /// All currently-live external identifiers (data-model §11).
    #[must_use]
    pub fn external_ids(&self) -> Vec<&ExternalId> {
        self.state.external_ids.iter().map(|e| &e.value).collect()
    }

    /// Currently-live partners, each paired with the `AssertionId` that introduced it — the read
    /// side of the per-row correction (Remove retracts it).
    #[must_use]
    pub fn partners_with_assertions(&self) -> &[Attributed<AssertedPartner>] {
        &self.state.partners
    }

    /// Currently-live children (membership), each paired with the `AssertionId` that introduced it —
    /// the read side of the per-row correction (Remove retracts it, cascading its relationships).
    #[must_use]
    pub fn children_with_assertions(&self) -> &[Attributed<AssertedChild>] {
        &self.state.children
    }

    /// Currently-live child–parent relationship rows, each paired with its introducing `AssertionId`
    /// — the read side of the per-link correction (Edit supersedes it, a clear retracts it, ADR 0021).
    #[must_use]
    pub fn child_relationships_with_assertions(&self) -> &[Attributed<Asserted<ChildRelationship>>] {
        &self.state.child_relationships
    }

    /// Currently-live linked family events, each paired with its introducing `AssertionId`.
    #[must_use]
    pub fn linked_events_with_assertions(&self) -> &[Attributed<AssertedFamilyEvent>] {
        &self.state.linked_events
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

impl View<FamilyState> for FamilyView {
    fn update(&mut self, event: &EventEnvelope<FamilyState>) {
        evolve(&mut self.state, &event.payload);
    }
}
