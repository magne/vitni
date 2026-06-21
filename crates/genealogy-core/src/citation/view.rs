//! [`CitationView`] — the conclusion-layer read model for a Citation (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use crate::citation::decide::evolve;
use crate::citation::state::CitationState;
use crate::date::GenealogicalDate;
use crate::ids::{CitationId, HumanId, SourceId};
use crate::provenance::{Confidence, EvidenceAnalysis};
use crate::text::Attribute;

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
        self.state.page.as_ref().map(|p| p.value.as_str())
    }

    /// The date of the cited record, if asserted.
    #[must_use]
    pub fn date(&self) -> Option<&GenealogicalDate> {
        self.state.date.as_ref().map(|d| &d.value)
    }

    /// The operator's confidence in the citation, if set.
    #[must_use]
    pub fn confidence(&self) -> Option<&Confidence> {
        self.state.confidence.as_ref().map(|c| &c.value)
    }

    /// The citation's evidence analysis, if set.
    #[must_use]
    pub fn evidence_analysis(&self) -> Option<&EvidenceAnalysis> {
        self.state.evidence_analysis.as_ref().map(|e| &e.value)
    }

    /// All currently-live attributes, in assertion order.
    #[must_use]
    pub fn attributes(&self) -> Vec<&Attribute> {
        self.state.attributes.iter().map(|a| &a.value).collect()
    }
}

impl View<CitationState> for CitationView {
    fn update(&mut self, event: &EventEnvelope<CitationState>) {
        evolve(&mut self.state, &event.payload);
    }
}
