//! [`PersonView`] — the conclusion-layer read model for a Person (data-model §6).
//!
//! The view is rebuilt by folding the same events as the aggregate (it delegates to `evolve`), so
//! corrections — retractions and supersessions — are reflected correctly. A denormalized SQL
//! read schema is deferred (ADR 0002, data-model §17); for now the view exposes its projected
//! fields through accessor methods over the folded state.

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use crate::enums::{EvidenceLevel, Sex};
use crate::fact::Fact;
use crate::ids::{HumanId, PersonId};
use crate::name::PersonName;
use crate::person::decide::evolve;
use crate::person::state::{Association, Participation, PersonState};
use crate::text::ExternalId;

/// The current best synthesis of a Person, derived from the event log (data-model §6).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonView {
    state: PersonState,
}

impl PersonView {
    /// Returns `true` once the person has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The person's id, once created.
    #[must_use]
    pub fn person_id(&self) -> Option<PersonId> {
        self.state.person_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// Whether the person is a persona or a conclusion.
    #[must_use]
    pub fn evidence_level(&self) -> Option<EvidenceLevel> {
        self.state.evidence_level
    }

    /// All currently-live asserted names (retracted ones are excluded).
    #[must_use]
    pub fn names(&self) -> Vec<&PersonName> {
        self.state.names.iter().map(|n| &n.value).collect()
    }

    /// The most recently asserted sex.
    #[must_use]
    pub fn sex(&self) -> Option<&Sex> {
        self.state.sex.as_ref().map(|s| &s.value)
    }

    /// All currently-live asserted facts.
    #[must_use]
    pub fn facts(&self) -> Vec<&Fact> {
        self.state.facts.iter().map(|f| &f.value).collect()
    }

    /// All currently-live asserted person-to-person associations (data-model §10).
    #[must_use]
    pub fn associations(&self) -> Vec<&Association> {
        self.state.associations.iter().map(|a| &a.value).collect()
    }

    /// All currently-live asserted event participations (data-model §6, §10).
    #[must_use]
    pub fn participations(&self) -> Vec<&Participation> {
        self.state.participations.iter().map(|p| &p.value).collect()
    }

    /// Whether the person is marked private.
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

impl View<PersonState> for PersonView {
    fn update(&mut self, event: &EventEnvelope<PersonState>) {
        evolve(&mut self.state, &event.payload);
    }
}
