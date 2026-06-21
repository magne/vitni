//! Place events — the past-tense assertions the aggregate produces (data-model §10).
//!
//! Each event carries its [`AssertionId`] and [`EventContext`] in the payload (ADR 0004 §1–§2);
//! the body is internally tagged (`type`) and flattened, so a stored event is one flat JSON object
//! (ADR 0004 §4).

use serde::{Deserialize, Serialize};

use crate::assertions::{Envelope, EventBody};
use crate::enums::PlaceType;
use crate::ids::{AssertionId, HumanId, PlaceId};
use crate::place_name::PlaceName;

/// A single Place assertion plus its provenance envelope (ADR 0004 §1).
pub type PlaceEvent = Envelope<PlaceEventBody>;

/// The Place claim variants (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PlaceEventBody {
    /// A place aggregate was created.
    PlaceCreated {
        /// The created place.
        place_id: PlaceId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// The place's type.
        place_type: PlaceType,
    },
    /// The place's type was set / changed.
    PlaceTypeSet {
        /// The place.
        place_id: PlaceId,
        /// The new place type.
        place_type: PlaceType,
    },
    /// A name was asserted for the place.
    NameAsserted {
        /// The place the name belongs to.
        place_id: PlaceId,
        /// The asserted name.
        name: PlaceName,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The place.
        place_id: PlaceId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The place.
        place_id: PlaceId,
        /// The assertion being superseded.
        target: AssertionId,
    },
}

impl EventBody for PlaceEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::PlaceCreated { .. } => "PlaceCreated",
            Self::PlaceTypeSet { .. } => "PlaceTypeSet",
            Self::NameAsserted { .. } => "NameAsserted",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
        }
    }

    fn version(&self) -> &'static str {
        // Per-variant; bumped only on an additive payload change (ADR 0004 §4).
        "1.0"
    }
}
