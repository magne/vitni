//! [`CitationView`] — the conclusion-layer read model for a Citation (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use crate::citation::decide::evolve;
use crate::citation::state::CitationState;
use crate::ids::{CitationId, HumanId, SourceId};

/// The current best synthesis of a Citation, derived from the event log (data-model §6).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationView {
    state: CitationState,
}

impl CitationView {
    /// Returns `true` once the citation has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The citation's id, once created.
    #[must_use]
    pub fn citation_id(&self) -> Option<CitationId> {
        self.state.citation_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// The source this citation points into.
    #[must_use]
    pub fn source_id(&self) -> Option<SourceId> {
        self.state.source_id
    }

    /// The page / locator within the source, if set.
    #[must_use]
    pub fn page(&self) -> Option<&str> {
        self.state.page.as_deref()
    }
}

impl View<CitationState> for CitationView {
    fn update(&mut self, event: &EventEnvelope<CitationState>) {
        evolve(&mut self.state, &event.payload);
    }
}
