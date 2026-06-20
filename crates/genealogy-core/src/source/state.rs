//! [`SourceState`] — the folded aggregate state used by the decision core.
//!
//! Asserted fields (the title) are kept attributed to the [`AssertionId`] that introduced them, so
//! a retraction or supersession can remove exactly the right entry (data-model §10).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::ids::{AssertionId, HumanId, SourceId};

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
        if self.title.as_ref().is_some_and(|t| t.assertion_id == target) {
            self.title = None;
        }
        self.live_assertions.remove(&target);
    }
}
