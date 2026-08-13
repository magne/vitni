//! [`CitationView`] — the conclusion-layer read model for a Citation (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::Attributed;
use crate::citation::decide::evolve;
use crate::citation::state::CitationState;
use crate::date::GenealogicalDate;
use crate::enums::Restriction;
use crate::ids::{CitationId, HumanId, NoteId, SourceId, TagId};
use crate::provenance::{Agent, Confidence, EvidenceAnalysis, Timestamp};
use crate::text::{Attribute, MediaRef};

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

    /// The operator who created the citation (the "asserted by" provenance) — typed as an [`Agent`]
    /// so the Human/Software/AiModel distinction survives to the UI (finding 7, ADR 0021 §4).
    #[must_use]
    pub fn created_by(&self) -> Option<&Agent> {
        self.state.created.as_ref().map(|c| &c.by)
    }

    /// When the citation was created, if known.
    #[must_use]
    pub fn created_at(&self) -> Option<Timestamp> {
        self.state.created.as_ref().map(|c| c.at)
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

    /// The citation's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }

    /// Currently-live attributes, each paired with the `AssertionId` that introduced it — the read
    /// side of the per-row correction (Edit supersedes it, Remove retracts it).
    #[must_use]
    pub fn attributes_with_assertions(&self) -> &[Attributed<Attribute>] {
        &self.state.attributes
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

impl View<CitationState> for CitationView {
    fn update(&mut self, event: &EventEnvelope<CitationState>) {
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
        let state = CitationState {
            notes: vec![Attributed {
                assertion_id: aid,
                value: note,
            }],
            ..Default::default()
        };
        let view = CitationView { state };
        assert_eq!(view.notes_with_assertions()[0].assertion_id, aid);
    }
}
