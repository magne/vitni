//! [`CitationState`] — the folded aggregate state used by the decision core.
//!
//! Asserted fields (the page) are kept attributed to the [`AssertionId`] that introduced them, so a
//! retraction or supersession can remove exactly the right entry (data-model §10).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::ids::{AssertionId, CitationId, HumanId, SourceId};

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
        if self.page.as_ref().is_some_and(|p| p.assertion_id == target) {
            self.page = None;
        }
        self.live_assertions.remove(&target);
    }
}
