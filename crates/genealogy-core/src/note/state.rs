//! [`NoteState`] — the folded aggregate state used by the decision core.
//!
//! Type and rich text are last-writer-wins, attributed to the [`AssertionId`] that introduced them;
//! tags are projected so the detail tabs can render them.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::enums::{NoteType, Restriction};
use crate::ids::{AssertionId, HumanId, NoteId, TagId};
use crate::text::RichText;

/// The folded state of a Note aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteState {
    /// Whether `NoteCreated` has been seen.
    pub exists: bool,
    /// The note's id (set on creation).
    pub note_id: Option<NoteId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// The note's type (last writer wins).
    pub note_type: Option<Attributed<NoteType>>,
    /// The note's rich-text content (last writer wins).
    pub text: Option<Attributed<RichText>>,
    /// All currently-applied tags.
    pub tags: Vec<Attributed<TagId>>,
    /// The note's privacy restrictions (GEDCOM `RESN`, last writer wins — data-model §6).
    pub restrictions: BTreeSet<Restriction>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl NoteState {
    /// Removes every value introduced by `target` and drops it from the live set.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        if self.note_type.as_ref().is_some_and(|t| t.assertion_id == target) {
            self.note_type = None;
        }
        if self.text.as_ref().is_some_and(|t| t.assertion_id == target) {
            self.text = None;
        }
        self.tags.retain(|t| t.assertion_id != target);
        self.live_assertions.remove(&target);
    }
}
