//! Place events — the past-tense assertions the aggregate produces (data-model §10).
//!
//! Each event carries its [`AssertionId`] and [`EventContext`] in the payload (ADR 0004 §1–§2);
//! the body is internally tagged (`type`) and flattened, so a stored event is one flat JSON object
//! (ADR 0004 §4).

use cqrs_es::DomainEvent;
use serde::{Deserialize, Serialize};

use crate::enums::PlaceType;
use crate::ids::{AssertionId, HumanId, PlaceId};
use crate::place_name::PlaceName;
use crate::provenance::{AssertionMeta, EventContext};

/// A single Place assertion plus its provenance envelope (ADR 0004 §1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceEvent {
    /// Identity of this assertion, so a correction can target it (ADR 0004 §2).
    pub assertion_id: AssertionId,
    /// Who / when / why / how sure / on what evidence (data-model §8).
    pub context: EventContext,
    /// The claim itself.
    #[serde(flatten)]
    pub body: PlaceEventBody,
}

impl PlaceEvent {
    /// Stamps `body` with the supplied assertion id and context (ADR 0004 §3).
    #[must_use]
    pub fn new(meta: &AssertionMeta, body: PlaceEventBody) -> Self {
        Self {
            assertion_id: meta.assertion_id,
            context: meta.context.clone(),
            body,
        }
    }
}

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
}

impl PlaceEventBody {
    /// The variant name, used as the `cqrs-es` event type (ADR 0004 §4).
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::PlaceCreated { .. } => "PlaceCreated",
            Self::PlaceTypeSet { .. } => "PlaceTypeSet",
            Self::NameAsserted { .. } => "NameAsserted",
        }
    }
}

impl DomainEvent for PlaceEvent {
    fn event_type(&self) -> String {
        self.body.type_name().to_owned()
    }

    fn event_version(&self) -> String {
        // Bumped only on an incompatible payload change (ADR 0004 §4).
        "1.0".to_owned()
    }
}
