//! [`SourceView`] — the conclusion-layer read model for a Source (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use crate::ids::{HumanId, SourceId};
use crate::source::decide::evolve;
use crate::source::state::SourceState;

/// The current best synthesis of a Source, derived from the event log (data-model §6).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceView {
    state: SourceState,
}

impl SourceView {
    /// Returns `true` once the source has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The source's id, once created.
    #[must_use]
    pub fn source_id(&self) -> Option<SourceId> {
        self.state.source_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// The bibliographic title, if set.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.state.title.as_deref()
    }
}

impl View<SourceState> for SourceView {
    fn update(&mut self, event: &EventEnvelope<SourceState>) {
        evolve(&mut self.state, &event.payload);
    }
}
