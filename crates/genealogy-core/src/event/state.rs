//! [`EventState`] — the folded aggregate state used by the decision core.
//!
//! Asserted fields (type, date, linked place) are kept attributed to the [`AssertionId`] that
//! introduced them, so a retraction or supersession can remove exactly the right entry
//! (data-model §10).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::date::GenealogicalDate;
use crate::enums::EventType;
use crate::ids::{AssertionId, EventId, HumanId, PlaceId};

/// The folded state of an Event aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct EventState {
    /// Whether `EventCreated` has been seen.
    pub exists: bool,
    /// The event's id (set on creation).
    pub event_id: Option<EventId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// The kind of event (last writer wins).
    pub event_type: Option<Attributed<EventType>>,
    /// When the event occurred (last writer wins).
    pub date: Option<Attributed<GenealogicalDate>>,
    /// Where the event occurred (last writer wins).
    pub place_id: Option<Attributed<PlaceId>>,
    /// Whether the event is private (Gramps' universal privacy flag; set on creation).
    pub private: bool,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl EventState {
    /// Removes every value introduced by `target` and drops it from the live set.
    ///
    /// This is the non-destructive-correction fold: the *event log* keeps the original assertion
    /// forever, but the derived state no longer reflects the retracted claim.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        if self.event_type.as_ref().is_some_and(|t| t.assertion_id == target) {
            self.event_type = None;
        }
        if self.date.as_ref().is_some_and(|d| d.assertion_id == target) {
            self.date = None;
        }
        if self.place_id.as_ref().is_some_and(|p| p.assertion_id == target) {
            self.place_id = None;
        }
        self.live_assertions.remove(&target);
    }
}
