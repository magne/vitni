//! [`PlaceState`] — the folded aggregate state used by the decision core.
//!
//! `Default` (an unseen place) and serializable (for snapshotting), rebuilt by replaying events
//! through `evolve`. Names accumulate; the type is last-writer-wins. The universal
//! retract/supersede correction pair is added across all aggregates in a later phase (roadmap
//! Phase 2), so this spike keeps the state minimal.

use serde::{Deserialize, Serialize};

use crate::enums::PlaceType;
use crate::ids::{HumanId, PlaceId};
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
    pub place_type: Option<PlaceType>,
    /// All asserted names, in assertion order.
    pub names: Vec<PlaceName>,
}
