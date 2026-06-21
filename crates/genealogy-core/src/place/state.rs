//! [`PlaceState`] — the folded aggregate state used by the decision core.
//!
//! `Default` (an unseen place) and serializable (for snapshotting), rebuilt by replaying events
//! through `evolve`. Names accumulate; the type is last-writer-wins. Both are kept attributed to
//! the [`AssertionId`] that introduced them, so a retraction or supersession can remove exactly the
//! right entry (data-model §10).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::enums::PlaceType;
use crate::ids::{AssertionId, HumanId, PlaceId};
use crate::place_name::PlaceName;

/// The folded state of a Place aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceState {
    /// Whether `PlaceCreated` has been seen.
    pub exists: bool,
    /// The place's id (set on creation).
    pub place_id: Option<PlaceId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// The place's type (last writer wins).
    pub place_type: Option<Attributed<PlaceType>>,
    /// All currently-live asserted names, in assertion order.
    pub names: Vec<Attributed<PlaceName>>,
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
        if self.place_type.as_ref().is_some_and(|t| t.assertion_id == target) {
            self.place_type = None;
        }
        self.live_assertions.remove(&target);
    }
}
