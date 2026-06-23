//! [`SourceView`] — the conclusion-layer read model for a Source (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::enums::Restriction;
use crate::ids::{HumanId, SourceId};
use crate::repo_ref::RepoRef;
use crate::source::decide::evolve;
use crate::source::state::SourceState;
use crate::text::Attribute;

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
        self.state.title.as_ref().map(|t| t.value.as_str())
    }

    /// The author, if set.
    #[must_use]
    pub fn author(&self) -> Option<&str> {
        self.state.author.as_ref().map(|a| a.value.as_str())
    }

    /// The publication info, if set.
    #[must_use]
    pub fn pub_info(&self) -> Option<&str> {
        self.state.pub_info.as_ref().map(|p| p.value.as_str())
    }

    /// The abbreviation, if set.
    #[must_use]
    pub fn abbrev(&self) -> Option<&str> {
        self.state.abbrev.as_ref().map(|a| a.value.as_str())
    }

    /// All currently-live repository links, in assertion order.
    #[must_use]
    pub fn repositories(&self) -> Vec<&RepoRef> {
        self.state.repositories.iter().map(|r| &r.value).collect()
    }

    /// All currently-live attributes, in assertion order.
    #[must_use]
    pub fn attributes(&self) -> Vec<&Attribute> {
        self.state.attributes.iter().map(|a| &a.value).collect()
    }

    /// The source's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }
}

impl View<SourceState> for SourceView {
    fn update(&mut self, event: &EventEnvelope<SourceState>) {
        evolve(&mut self.state, &event.payload);
    }
}
