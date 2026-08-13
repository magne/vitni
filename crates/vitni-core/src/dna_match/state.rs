//! [`DnaMatchState`] — the folded aggregate state used by the decision core.
//!
//! The observation fields (tests, provider, shared cM, …) are set once on `DnaMatchObserved`.
//! Segments and shared-ancestors accumulate (attributed); the confirmation status is
//! last-writer-wins (attributed). Notes and tags are projected (mirroring Place) so the detail tabs
//! and the cross-aggregate note index can render them (Phase 5 PR 11).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::dna::{Centimorgans, DnaProvider, DnaSegment, PercentShared, SharedAncestor};
use crate::enums::Restriction;
use crate::ids::{AssertionId, DnaMatchId, DnaTestId, HumanId, NoteId, TagId};

/// Whether a human has confirmed or rejected a match (data-model §12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum MatchStatus {
    /// The match was confirmed.
    Confirmed,
    /// The match was rejected.
    Rejected,
}

/// The folded state of a `DnaMatch` aggregate (data-model §6, §12).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnaMatchState {
    /// Whether `DnaMatchObserved` has been seen.
    pub exists: bool,
    /// The match's id (set on observation).
    pub dna_match_id: Option<DnaMatchId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// One side's test (set on observation).
    pub test_a: Option<DnaTestId>,
    /// The other side's test (set on observation).
    pub test_b: Option<DnaTestId>,
    /// The provider (set on observation).
    pub provider: Option<DnaProvider>,
    /// Total shared centimorgans (set on observation).
    pub shared_cm: Option<Centimorgans>,
    /// Shared percentage (set on observation).
    pub percent_shared: Option<PercentShared>,
    /// The number of shared segments reported (set on observation).
    pub segment_count: Option<u32>,
    /// The largest shared segment's length (set on observation).
    pub largest_segment_cm: Option<Centimorgans>,
    /// The provider's predicted relationship (set on observation).
    pub predicted_relationship: Option<String>,
    /// All currently-live segments, in assertion order.
    pub segments: Vec<Attributed<DnaSegment>>,
    /// All currently-live shared ancestors, in assertion order.
    pub shared_ancestors: Vec<Attributed<SharedAncestor>>,
    /// The confirmation status (last writer wins).
    pub status: Option<Attributed<MatchStatus>>,
    /// All currently-live attached notes.
    pub notes: Vec<Attributed<NoteId>>,
    /// All currently-applied tags.
    pub tags: Vec<Attributed<TagId>>,
    /// The match's privacy restrictions (GEDCOM `RESN`, last writer wins — data-model §6).
    pub restrictions: BTreeSet<Restriction>,
    /// The assertion that set the current `restrictions`, so retracting it clears them (the set is
    /// replaced wholesale, not accumulated, so it cannot be attributed per-element — ADR 0021 §3).
    #[serde(default)]
    pub restrictions_assertion: Option<AssertionId>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl DnaMatchState {
    /// Removes every value introduced by `target` and drops it from the live set.
    ///
    /// The observation fields are set once on create and are not removed by a correction (a
    /// retraction of the observation would retract the whole match — out of scope here); only the
    /// accumulating and last-writer-wins assertions are reverted.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        self.segments.retain(|s| s.assertion_id != target);
        self.shared_ancestors.retain(|a| a.assertion_id != target);
        if self.status.as_ref().is_some_and(|s| s.assertion_id == target) {
            self.status = None;
        }
        self.notes.retain(|n| n.assertion_id != target);
        self.tags.retain(|t| t.assertion_id != target);
        if self.restrictions_assertion == Some(target) {
            self.restrictions.clear();
            self.restrictions_assertion = None;
        }
        self.live_assertions.remove(&target);
    }
}
