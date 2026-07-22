//! [`PlaceState`] — the folded aggregate state used by the decision core.
//!
//! `Default` (an unseen place) and serializable (for snapshotting), rebuilt by replaying events
//! through `evolve`. Names and enclosures accumulate; the type, code, and coordinates are
//! last-writer-wins. Each projected value is kept attributed to the [`AssertionId`] that introduced
//! it, so a retraction or supersession can remove exactly the right entry (data-model §10), and the
//! provenance-bearing facts ([`Asserted`]) carry their surety + backing citations for the read model.
//! Attachment claims (citations, media, notes, tags) are projected so the detail tabs can render them.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::{Asserted, Attributed};
use crate::enums::{PlaceType, Restriction};
use crate::geo::GeoCoordinates;
use crate::ids::{AssertionId, CitationId, HumanId, NoteId, PlaceId, TagId};
use crate::place_geometry::PlaceGeometryAssertion;
use crate::place_name::PlaceName;
use crate::place_ref::PlaceRef;
use crate::text::MediaRef;

/// The folded state of a Place aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceState {
    /// Whether `PlaceCreated` has been seen.
    pub exists: bool,
    /// The place's id (set on creation).
    pub place_id: Option<PlaceId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// The place's type (last writer wins), with its provenance.
    pub place_type: Option<Attributed<Asserted<PlaceType>>>,
    /// All currently-live asserted names, in assertion order, each with its provenance.
    pub names: Vec<Attributed<Asserted<PlaceName>>>,
    /// All currently-live enclosing-place relationships, in assertion order, each with its provenance.
    pub enclosed_by: Vec<Attributed<Asserted<PlaceRef>>>,
    /// The place's coordinates (last writer wins), with its provenance.
    pub coordinates: Option<Attributed<Asserted<GeoCoordinates>>>,
    /// All currently-live dated geometry assertions, in assertion order, each with its provenance —
    /// these accumulate rather than replace (ADR 0024), unlike `coordinates` above.
    #[serde(default)]
    pub geometries: Vec<Attributed<Asserted<PlaceGeometryAssertion>>>,
    /// The place's code (last writer wins), with its provenance.
    pub code: Option<Attributed<Asserted<String>>>,
    /// All currently-live citations backing the place's claims.
    pub citations: Vec<Attributed<CitationId>>,
    /// All currently-live attached media.
    pub media: Vec<Attributed<MediaRef>>,
    /// All currently-live attached notes.
    pub notes: Vec<Attributed<NoteId>>,
    /// All currently-applied tags.
    pub tags: Vec<Attributed<TagId>>,
    /// The place's privacy restrictions (GEDCOM `RESN`, last writer wins — data-model §6).
    pub restrictions: BTreeSet<Restriction>,
    /// The assertion that set the current `restrictions`, so retracting it clears them (the set is
    /// replaced wholesale, not accumulated, so it cannot be attributed per-element — ADR 0021 §3).
    #[serde(default)]
    pub restrictions_assertion: Option<AssertionId>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl PlaceState {
    /// Removes every value introduced by `target` and drops it from the live set.
    ///
    /// This is the non-destructive-correction fold: the *event log* keeps the original assertion
    /// forever, but the derived state no longer reflects the retracted claim.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        self.names.retain(|n| n.assertion_id != target);
        self.enclosed_by.retain(|e| e.assertion_id != target);
        if self.place_type.as_ref().is_some_and(|t| t.assertion_id == target) {
            self.place_type = None;
        }
        if self.coordinates.as_ref().is_some_and(|c| c.assertion_id == target) {
            self.coordinates = None;
        }
        self.geometries.retain(|g| g.assertion_id != target);
        if self.code.as_ref().is_some_and(|c| c.assertion_id == target) {
            self.code = None;
        }
        self.citations.retain(|c| c.assertion_id != target);
        self.media.retain(|m| m.assertion_id != target);
        self.notes.retain(|n| n.assertion_id != target);
        self.tags.retain(|t| t.assertion_id != target);
        if self.restrictions_assertion == Some(target) {
            self.restrictions.clear();
            self.restrictions_assertion = None;
        }
        self.live_assertions.remove(&target);
    }
}
