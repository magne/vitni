//! [`MediaView`] — the conclusion-layer read model for a Media object (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use crate::date::GenealogicalDate;
use crate::ids::{HumanId, MediaId};
use crate::media::decide::evolve;
use crate::media::state::MediaState;
use crate::media_path::MediaPath;
use crate::text::Attribute;

/// The current best synthesis of a Media object, derived from the event log (data-model §6).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct MediaView {
    state: MediaState,
}

impl MediaView {
    /// Returns `true` once the media has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The media's id, once created.
    #[must_use]
    pub fn media_id(&self) -> Option<MediaId> {
        self.state.media_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// The media's location, if set.
    #[must_use]
    pub fn path(&self) -> Option<&MediaPath> {
        self.state.path.as_ref().map(|p| &p.value)
    }

    /// The media's checksum, if set.
    #[must_use]
    pub fn checksum(&self) -> Option<&str> {
        self.state.checksum.as_ref().map(|c| c.value.as_str())
    }

    /// The media's date, if asserted.
    #[must_use]
    pub fn date(&self) -> Option<&GenealogicalDate> {
        self.state.date.as_ref().map(|d| &d.value)
    }

    /// All currently-live attributes, in assertion order.
    #[must_use]
    pub fn attributes(&self) -> Vec<&Attribute> {
        self.state.attributes.iter().map(|a| &a.value).collect()
    }
}

impl View<MediaState> for MediaView {
    fn update(&mut self, event: &EventEnvelope<MediaState>) {
        evolve(&mut self.state, &event.payload);
    }
}
