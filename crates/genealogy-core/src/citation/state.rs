//! [`CitationState`] — the folded aggregate state used by the decision core.
//!
//! Asserted single-valued fields (page, date, confidence, evidence analysis) are last-writer-wins;
//! attributes, attached media/notes, and tags accumulate. Each is kept attributed to the
//! [`AssertionId`] that introduced it, so a retraction or supersession can remove exactly the right
//! entry (data-model §10, the Person precedent — ADR 0009 §4).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::date::GenealogicalDate;
use crate::enums::Restriction;
use crate::ids::{AssertionId, CitationId, HumanId, NoteId, SourceId, TagId};
use crate::provenance::{Confidence, EvidenceAnalysis};
use crate::text::{Attribute, MediaRef};

/// The folded state of a Citation aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct CitationState {
    /// Whether `CitationCreated` has been seen.
    pub exists: bool,
    /// The citation's id (set on creation).
    pub citation_id: Option<CitationId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// The source this citation points into (set on creation).
    pub source_id: Option<SourceId>,
    /// The page / locator within the source (last writer wins).
    pub page: Option<Attributed<String>>,
    /// The date of the cited record (last writer wins).
    pub date: Option<Attributed<GenealogicalDate>>,
    /// The operator's confidence in this citation (last writer wins).
    pub confidence: Option<Attributed<Confidence>>,
    /// The citation's evidence analysis (last writer wins).
    pub evidence_analysis: Option<Attributed<EvidenceAnalysis>>,
    /// All currently-live attributes, in assertion order.
    pub attributes: Vec<Attributed<Attribute>>,
    /// All currently-live attached media (e.g. `SOUR.OBJE`).
    pub media: Vec<Attributed<MediaRef>>,
    /// All currently-live attached notes.
    pub notes: Vec<Attributed<NoteId>>,
    /// All currently-applied tags.
    pub tags: Vec<Attributed<TagId>>,
    /// The citation's privacy restrictions (GEDCOM `RESN`, last writer wins — data-model §6).
    pub restrictions: BTreeSet<Restriction>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl CitationState {
    /// Removes every value introduced by `target` and drops it from the live set.
    ///
    /// This is the non-destructive-correction fold: the *event log* keeps the original assertion
    /// forever, but the derived state no longer reflects the retracted claim.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        self.attributes.retain(|a| a.assertion_id != target);
        self.media.retain(|m| m.assertion_id != target);
        self.notes.retain(|n| n.assertion_id != target);
        self.tags.retain(|t| t.assertion_id != target);
        if self.page.as_ref().is_some_and(|p| p.assertion_id == target) {
            self.page = None;
        }
        if self.date.as_ref().is_some_and(|d| d.assertion_id == target) {
            self.date = None;
        }
        if self.confidence.as_ref().is_some_and(|c| c.assertion_id == target) {
            self.confidence = None;
        }
        if self
            .evidence_analysis
            .as_ref()
            .is_some_and(|e| e.assertion_id == target)
        {
            self.evidence_analysis = None;
        }
        self.live_assertions.remove(&target);
    }
}
