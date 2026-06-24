//! [`SourceState`] — the folded aggregate state used by the decision core.
//!
//! Title/author/pub-info/abbrev are last-writer-wins; repositories, attributes, media, notes, and
//! tags accumulate. Each is kept attributed to the [`AssertionId`] that introduced it, so a
//! correction can remove exactly the right entry. Repository links additionally carry their
//! provenance (`Asserted`) so the read model can surface a link's surety + backing citations.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::{Asserted, Attributed};
use crate::enums::Restriction;
use crate::ids::{AssertionId, HumanId, NoteId, SourceId, TagId};
use crate::repo_ref::RepoRef;
use crate::text::{Attribute, MediaRef};

/// The folded state of a Source aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceState {
    /// Whether `SourceCreated` has been seen.
    pub exists: bool,
    /// The source's id (set on creation).
    pub source_id: Option<SourceId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// The bibliographic title (last writer wins).
    pub title: Option<Attributed<String>>,
    /// The author (last writer wins).
    pub author: Option<Attributed<String>>,
    /// The publication info (last writer wins).
    pub pub_info: Option<Attributed<String>>,
    /// The abbreviation (last writer wins).
    pub abbrev: Option<Attributed<String>>,
    /// All currently-live repository links, in assertion order, each with its provenance (surety +
    /// backing citations).
    pub repositories: Vec<Attributed<Asserted<RepoRef>>>,
    /// All currently-live attributes, in assertion order.
    pub attributes: Vec<Attributed<Attribute>>,
    /// All currently-live attached media, in assertion order.
    pub media: Vec<Attributed<MediaRef>>,
    /// All currently-live attached notes, in assertion order.
    pub notes: Vec<Attributed<NoteId>>,
    /// All currently-applied tags, in assertion order.
    pub tags: Vec<Attributed<TagId>>,
    /// The source's privacy restrictions (GEDCOM `RESN`, last writer wins — data-model §6).
    pub restrictions: BTreeSet<Restriction>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl SourceState {
    /// Removes every value introduced by `target` and drops it from the live set.
    ///
    /// This is the non-destructive-correction fold: the *event log* keeps the original assertion
    /// forever, but the derived state no longer reflects the retracted claim.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        self.repositories.retain(|r| r.assertion_id != target);
        self.attributes.retain(|a| a.assertion_id != target);
        self.media.retain(|m| m.assertion_id != target);
        self.notes.retain(|n| n.assertion_id != target);
        self.tags.retain(|t| t.assertion_id != target);
        if self.title.as_ref().is_some_and(|t| t.assertion_id == target) {
            self.title = None;
        }
        if self.author.as_ref().is_some_and(|a| a.assertion_id == target) {
            self.author = None;
        }
        if self.pub_info.as_ref().is_some_and(|p| p.assertion_id == target) {
            self.pub_info = None;
        }
        if self.abbrev.as_ref().is_some_and(|a| a.assertion_id == target) {
            self.abbrev = None;
        }
        self.live_assertions.remove(&target);
    }
}
