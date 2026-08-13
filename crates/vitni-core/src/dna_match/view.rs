//! [`DnaMatchView`] — the conclusion-layer read model for a `DnaMatch` (data-model §6, §12).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::Attributed;
use crate::dna::{Centimorgans, DnaProvider, DnaSegment, PercentShared, SharedAncestor};
use crate::dna_match::decide::evolve;
use crate::dna_match::state::{DnaMatchState, MatchStatus};
use crate::enums::Restriction;
use crate::ids::{DnaMatchId, DnaTestId, HumanId, NoteId, TagId};

/// The current best synthesis of a `DnaMatch`, derived from the event log (data-model §6, §12).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnaMatchView {
    state: DnaMatchState,
}

impl DnaMatchView {
    /// Returns `true` once the match has been observed.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The match's id, once observed.
    #[must_use]
    pub fn dna_match_id(&self) -> Option<DnaMatchId> {
        self.state.dna_match_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// One side's test.
    #[must_use]
    pub fn test_a(&self) -> Option<DnaTestId> {
        self.state.test_a
    }

    /// The other side's test.
    #[must_use]
    pub fn test_b(&self) -> Option<DnaTestId> {
        self.state.test_b
    }

    /// The provider the match was observed at.
    #[must_use]
    pub fn provider(&self) -> Option<&DnaProvider> {
        self.state.provider.as_ref()
    }

    /// Total shared centimorgans.
    #[must_use]
    pub fn shared_cm(&self) -> Option<Centimorgans> {
        self.state.shared_cm
    }

    /// Shared percentage, if reported.
    #[must_use]
    pub fn percent_shared(&self) -> Option<PercentShared> {
        self.state.percent_shared
    }

    /// The number of shared segments reported.
    #[must_use]
    pub fn segment_count(&self) -> Option<u32> {
        self.state.segment_count
    }

    /// The largest shared segment's length, if reported.
    #[must_use]
    pub fn largest_segment_cm(&self) -> Option<Centimorgans> {
        self.state.largest_segment_cm
    }

    /// The provider's predicted relationship, if any.
    #[must_use]
    pub fn predicted_relationship(&self) -> Option<&str> {
        self.state.predicted_relationship.as_deref()
    }

    /// All currently-live segments, in assertion order.
    #[must_use]
    pub fn segments(&self) -> Vec<&DnaSegment> {
        self.state.segments.iter().map(|s| &s.value).collect()
    }

    /// All currently-live shared ancestors, in assertion order.
    #[must_use]
    pub fn shared_ancestors(&self) -> Vec<&SharedAncestor> {
        self.state.shared_ancestors.iter().map(|a| &a.value).collect()
    }

    /// The confirmation status, if confirmed or rejected.
    #[must_use]
    pub fn status(&self) -> Option<MatchStatus> {
        self.state.status.as_ref().map(|s| s.value)
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

    /// The match's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }

    /// Currently-live segments, each paired with the `AssertionId` that introduced it — the read side
    /// of the per-row correction (Edit supersedes it, Remove retracts it).
    #[must_use]
    pub fn segments_with_assertions(&self) -> &[Attributed<DnaSegment>] {
        &self.state.segments
    }

    /// Currently-live shared ancestors, each paired with its introducing `AssertionId`.
    #[must_use]
    pub fn shared_ancestors_with_assertions(&self) -> &[Attributed<SharedAncestor>] {
        &self.state.shared_ancestors
    }

    /// Currently-live attached notes, each paired with the attach `AssertionId` (the detach target).
    #[must_use]
    pub fn notes_with_assertions(&self) -> &[Attributed<NoteId>] {
        &self.state.notes
    }
}

impl View<DnaMatchState> for DnaMatchView {
    fn update(&mut self, event: &EventEnvelope<DnaMatchState>) {
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
        let state = DnaMatchState {
            notes: vec![Attributed {
                assertion_id: aid,
                value: note,
            }],
            ..Default::default()
        };
        let view = DnaMatchView { state };
        assert_eq!(view.notes_with_assertions()[0].assertion_id, aid);
    }
}
