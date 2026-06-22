//! [`FamilyView`] — the conclusion-layer read model for a Family (data-model §6).
//!
//! The view is rebuilt by folding the same events as the aggregate (it delegates to `evolve`), so
//! corrections — retractions and supersessions — are reflected correctly. A denormalized SQL read
//! schema is deferred (ADR 0002, data-model §17); for now the view exposes its projected fields
//! through accessor methods over the folded state.

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use crate::family::decide::evolve;
use crate::family::state::{ChildEntry, FamilyState};
use crate::ids::{CitationId, FamilyId, HumanId, NoteId, PersonId, TagId};
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
        self.state.partners.iter().map(|p| p.value).collect()
    }

    /// All currently-live children (retracted ones are excluded).
    #[must_use]
    pub fn children(&self) -> Vec<&ChildEntry> {
        self.state.children.iter().map(|c| &c.value).collect()
    }

    /// All currently-live citations backing the family's claims, in assertion order.
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

    /// Whether the family is marked private.
    #[must_use]
    pub fn is_private(&self) -> bool {
        self.state.private
    }

    /// All currently-live external identifiers (data-model §11).
    #[must_use]
    pub fn external_ids(&self) -> Vec<&ExternalId> {
        self.state.external_ids.iter().map(|e| &e.value).collect()
    }
}

impl View<FamilyState> for FamilyView {
    fn update(&mut self, event: &EventEnvelope<FamilyState>) {
        evolve(&mut self.state, &event.payload);
    }
}
