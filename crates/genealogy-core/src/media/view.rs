//! [`MediaView`] — the conclusion-layer read model for a Media object (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::Attributed;
use crate::date::GenealogicalDate;
use crate::enums::Restriction;
use crate::ids::{CitationId, HumanId, MediaId, NoteId, TagId};
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

    /// The media's MIME type, if set.
    #[must_use]
    pub fn mime(&self) -> Option<&str> {
        self.state.mime.as_ref().map(|m| m.value.as_str())
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

    /// All currently-live citations backing the media's claims, in assertion order.
    #[must_use]
    pub fn citations(&self) -> Vec<CitationId> {
        self.state.citations.iter().map(|c| c.value).collect()
    }

    /// All currently-live attached notes, in assertion order.
    #[must_use]
    pub fn notes(&self) -> Vec<NoteId> {
        self.state.notes.iter().map(|n| n.value).collect()
    }

    /// All currently-applied tags, in assertion order.
    #[must_use]
    pub fn tags(&self) -> Vec<TagId> {
        self.state.tags.iter().map(|t| t.value).collect()
    }

    /// The media's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }

    /// Currently-live attributes, each paired with the `AssertionId` that introduced it — the read
    /// side of the per-row correction (Edit supersedes it, Remove retracts it).
    #[must_use]
    pub fn attributes_with_assertions(&self) -> &[Attributed<Attribute>] {
        &self.state.attributes
    }

    /// Currently-live citations, each paired with its introducing `AssertionId`.
    #[must_use]
    pub fn citations_with_assertions(&self) -> &[Attributed<CitationId>] {
        &self.state.citations
    }

    /// Currently-live attached notes, each paired with the attach `AssertionId` (the detach target).
    #[must_use]
    pub fn notes_with_assertions(&self) -> &[Attributed<NoteId>] {
        &self.state.notes
    }
}

impl View<MediaState> for MediaView {
    fn update(&mut self, event: &EventEnvelope<MediaState>) {
        evolve(&mut self.state, &event.payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assertions::Attributed;
    use crate::ids::AssertionId;
    use uuid::Uuid;

    #[test]
    fn notes_with_assertions_exposes_the_attach_assertion() {
        let aid = AssertionId::from_uuid(Uuid::from_u128(7));
        let note = crate::ids::NoteId::from_uuid(Uuid::from_u128(8));
        let state = MediaState {
            notes: vec![Attributed {
                assertion_id: aid,
                value: note,
            }],
            ..Default::default()
        };
        let view = MediaView { state };
        assert_eq!(view.notes_with_assertions()[0].assertion_id, aid);
    }
}
