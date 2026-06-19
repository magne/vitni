//! [`EventState`] — the folded aggregate state used by the decision core.

use serde::{Deserialize, Serialize};

use crate::date::GenealogicalDate;
use crate::enums::EventType;
use crate::ids::{EventId, HumanId, PlaceId};

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
    pub event_type: Option<EventType>,
    /// When the event occurred (last writer wins).
    pub date: Option<GenealogicalDate>,
    /// Where the event occurred (last writer wins).
    pub place_id: Option<PlaceId>,
}
