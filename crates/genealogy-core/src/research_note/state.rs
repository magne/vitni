//! [`ResearchNoteState`] — the folded aggregate state used by the decision core (ADR 0028).
//!
//! The body is last-writer-wins, attributed to the [`AssertionId`] that introduced it; tags are
//! projected so the detail tabs can render them; `subjects` is a plain (unattributed) non-empty set —
//! membership grows/shrinks via `AddSubject`/`RemoveSubject`, and `decide` refuses to shrink it past
//! one (a `ResearchNote` always names at least one subject, ADR 0028 §2).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::enums::Restriction;
use crate::ids::{AssertionId, HumanId, ResearchNoteId, TagId};
use crate::research_note::subject::SubjectRef;
use crate::text::RichText;

/// The folded state of a `ResearchNote` aggregate (ADR 0028 §2).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchNoteState {
    /// Whether `ResearchNoteCreated` has been seen.
    pub exists: bool,
    /// The research note's id (set on creation).
    pub research_note_id: Option<ResearchNoteId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// The conclusion-bearing entities this argument is about — non-empty once `exists` is `true`
    /// (ADR 0028 §2).
    pub subjects: BTreeSet<SubjectRef>,
    /// The optional short title (immutable once created).
    pub title: Option<String>,
    /// The note's written argument (last writer wins).
    pub body: Option<Attributed<RichText>>,
    /// All currently-applied tags.
    pub tags: Vec<Attributed<TagId>>,
    /// The note's privacy restrictions (GEDCOM `RESN`, last writer wins — data-model §6).
    pub restrictions: BTreeSet<Restriction>,
    /// The assertion that set the current `restrictions`, so retracting it clears them (the set is
    /// replaced wholesale, not accumulated, so it cannot be attributed per-element — ADR 0021 §3).
    pub restrictions_assertion: Option<AssertionId>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl ResearchNoteState {
    /// Removes every value introduced by `target` and drops it from the live set.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        if self.body.as_ref().is_some_and(|b| b.assertion_id == target) {
            self.body = None;
        }
        self.tags.retain(|t| t.assertion_id != target);
        if self.restrictions_assertion == Some(target) {
            self.restrictions.clear();
            self.restrictions_assertion = None;
        }
        self.live_assertions.remove(&target);
    }
}
