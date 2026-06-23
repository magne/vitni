//! [`TagView`] — the conclusion-layer read model for a Tag (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009). A Tag has no `HumanId`; it is
//! looked up by its aggregate id.

use std::collections::BTreeSet;

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use crate::enums::Restriction;
use crate::ids::TagId;
use crate::tag::decide::evolve;
use crate::tag::state::TagState;

/// The current best synthesis of a Tag, derived from the event log (data-model §6).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct TagView {
    state: TagState,
}

impl TagView {
    /// Returns `true` once the tag has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The tag's id, once created.
    #[must_use]
    pub fn tag_id(&self) -> Option<TagId> {
        self.state.tag_id
    }

    /// The tag's name, if set.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.state.name.as_deref()
    }

    /// The tag's colour, if set.
    #[must_use]
    pub fn color(&self) -> Option<&str> {
        self.state.color.as_deref()
    }

    /// The tag's sort priority, if set.
    #[must_use]
    pub fn priority(&self) -> Option<i32> {
        self.state.priority
    }

    /// The tag's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }
}

impl View<TagState> for TagView {
    fn update(&mut self, event: &EventEnvelope<TagState>) {
        evolve(&mut self.state, &event.payload);
    }
}
