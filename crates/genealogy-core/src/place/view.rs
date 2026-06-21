//! [`PlaceView`] — the conclusion-layer read model for a Place (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (it delegates to `evolve`). The denormalized
//! SQL read schema is deferred (ADR 0009); the view exposes its projected fields through accessors
//! over the folded state.

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use crate::enums::PlaceType;
use crate::ids::{HumanId, PlaceId};
use crate::place::decide::evolve;
use crate::place::state::PlaceState;
use crate::place_name::PlaceName;

/// The current best synthesis of a Place, derived from the event log (data-model §6).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceView {
    state: PlaceState,
}

impl PlaceView {
    /// Returns `true` once the place has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The place's id, once created.
    #[must_use]
    pub fn place_id(&self) -> Option<PlaceId> {
        self.state.place_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// The place's type.
    #[must_use]
    pub fn place_type(&self) -> Option<&PlaceType> {
        self.state.place_type.as_ref().map(|t| &t.value)
    }

    /// All currently-live asserted names, in assertion order.
    #[must_use]
    pub fn names(&self) -> Vec<&PlaceName> {
        self.state.names.iter().map(|n| &n.value).collect()
    }
}

impl View<PlaceState> for PlaceView {
    fn update(&mut self, event: &EventEnvelope<PlaceState>) {
        evolve(&mut self.state, &event.payload);
    }
}
