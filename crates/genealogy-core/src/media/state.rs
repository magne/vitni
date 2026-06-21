//! [`MediaState`] — the folded aggregate state used by the decision core.
//!
//! Path, checksum, and date are last-writer-wins; attributes accumulate. Each is attributed to the
//! [`AssertionId`] that introduced it. Citations, notes, and tags register only in `live_assertions`
//! (the Person precedent — ADR 0009 §4).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::date::GenealogicalDate;
use crate::ids::{AssertionId, HumanId, MediaId};
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
    /// The media's date (last writer wins).
    pub date: Option<Attributed<GenealogicalDate>>,
    /// All currently-live attributes, in assertion order.
    pub attributes: Vec<Attributed<Attribute>>,
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
        if self.date.as_ref().is_some_and(|d| d.assertion_id == target) {
            self.date = None;
        }
        self.live_assertions.remove(&target);
    }
}
