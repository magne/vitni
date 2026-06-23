//! [`DnaTestView`] — the conclusion-layer read model for a `DnaTest` (data-model §6, §12).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::dna::{DnaGenomeBuild, DnaProvider, DnaTestType};
use crate::dna_test::decide::evolve;
use crate::dna_test::state::DnaTestState;
use crate::enums::Restriction;
use crate::ids::{DnaTestId, HumanId, PersonId};

/// The current best synthesis of a `DnaTest`, derived from the event log (data-model §6, §12).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnaTestView {
    state: DnaTestState,
}

impl DnaTestView {
    /// Returns `true` once the test has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The test's id, once created.
    #[must_use]
    pub fn dna_test_id(&self) -> Option<DnaTestId> {
        self.state.dna_test_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// The person this test belongs to.
    #[must_use]
    pub fn person_id(&self) -> Option<PersonId> {
        self.state.person_id
    }

    /// The testing provider, if set.
    #[must_use]
    pub fn provider(&self) -> Option<&DnaProvider> {
        self.state.provider.as_ref().map(|p| &p.value)
    }

    /// The provider's kit id, if set.
    #[must_use]
    pub fn kit_id(&self) -> Option<&str> {
        self.state.kit_id.as_ref().map(|k| k.value.as_str())
    }

    /// The test type, if set.
    #[must_use]
    pub fn test_type(&self) -> Option<DnaTestType> {
        self.state.test_type.as_ref().map(|t| t.value)
    }

    /// The genome build, if set.
    #[must_use]
    pub fn genome_build(&self) -> Option<DnaGenomeBuild> {
        self.state.genome_build.as_ref().map(|g| g.value)
    }

    /// All currently-live haplogroups, in assertion order.
    #[must_use]
    pub fn haplogroups(&self) -> Vec<&str> {
        self.state.haplogroups.iter().map(|h| h.value.as_str()).collect()
    }

    /// The test's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }
}

impl View<DnaTestState> for DnaTestView {
    fn update(&mut self, event: &EventEnvelope<DnaTestState>) {
        evolve(&mut self.state, &event.payload);
    }
}
