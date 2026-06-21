//! [`NoteState`] — the folded aggregate state used by the decision core.
//!
//! Type and rich text are last-writer-wins, attributed to the [`AssertionId`] that introduced them;
//! tags register only in `live_assertions` (the Person precedent — ADR 0009 §4).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::enums::NoteType;
use crate::ids::{AssertionId, HumanId, NoteId};
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
        self.live_assertions.remove(&target);
    }
}
