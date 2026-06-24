//! [`MediaState`] — the folded aggregate state used by the decision core.
//!
//! Path, checksum, MIME, and date are last-writer-wins; attributes accumulate. Each is attributed to
//! the [`AssertionId`] that introduced it. Citations, notes, and tags are projected so the detail tabs
//! can render them.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::date::GenealogicalDate;
use crate::enums::Restriction;
use crate::ids::{AssertionId, CitationId, HumanId, MediaId, NoteId, TagId};
use crate::media_path::MediaPath;
use crate::text::Attribute;

/// The folded state of a Media aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaState {
    /// Whether `MediaCreated` has been seen.
    pub exists: bool,
    /// The media's id (set on creation).
    pub media_id: Option<MediaId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// The media's location (last writer wins).
    pub path: Option<Attributed<MediaPath>>,
    /// The media's checksum (last writer wins).
    pub checksum: Option<Attributed<String>>,
    /// The media's MIME type (last writer wins).
    pub mime: Option<Attributed<String>>,
    /// The media's date (last writer wins).
    pub date: Option<Attributed<GenealogicalDate>>,
    /// All currently-live attributes, in assertion order.
    pub attributes: Vec<Attributed<Attribute>>,
    /// All currently-live citations backing the media's claims.
    pub citations: Vec<Attributed<CitationId>>,
    /// All currently-live attached notes.
    pub notes: Vec<Attributed<NoteId>>,
    /// All currently-applied tags.
    pub tags: Vec<Attributed<TagId>>,
    /// The media's privacy restrictions (GEDCOM `RESN`, last writer wins — data-model §6).
    pub restrictions: BTreeSet<Restriction>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl MediaState {
    /// Removes every value introduced by `target` and drops it from the live set.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        self.attributes.retain(|a| a.assertion_id != target);
        if self.path.as_ref().is_some_and(|p| p.assertion_id == target) {
            self.path = None;
        }
        if self.checksum.as_ref().is_some_and(|c| c.assertion_id == target) {
            self.checksum = None;
        }
        if self.mime.as_ref().is_some_and(|m| m.assertion_id == target) {
            self.mime = None;
        }
        if self.date.as_ref().is_some_and(|d| d.assertion_id == target) {
            self.date = None;
        }
        self.citations.retain(|c| c.assertion_id != target);
        self.notes.retain(|n| n.assertion_id != target);
        self.tags.retain(|t| t.assertion_id != target);
        self.live_assertions.remove(&target);
    }
}
