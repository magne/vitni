//! [`NoteView`] — the conclusion-layer read model for a Note (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::enums::{NoteType, Restriction};
use crate::ids::{HumanId, NoteId, TagId};
use crate::note::decide::evolve;
use crate::note::state::NoteState;
use crate::text::RichText;

/// The current best synthesis of a Note, derived from the event log (data-model §6).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct NoteView {
    state: NoteState,
}

impl NoteView {
    /// Returns `true` once the note has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The note's id, once created.
    #[must_use]
    pub fn note_id(&self) -> Option<NoteId> {
        self.state.note_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// The note's type, if set.
    #[must_use]
    pub fn note_type(&self) -> Option<&NoteType> {
        self.state.note_type.as_ref().map(|t| &t.value)
    }

    /// The note's rich-text content, if set.
    #[must_use]
    pub fn text(&self) -> Option<&RichText> {
        self.state.text.as_ref().map(|t| &t.value)
    }

    /// All currently-applied tags, in assertion order.
    #[must_use]
    pub fn tags(&self) -> Vec<TagId> {
        self.state.tags.iter().map(|t| t.value).collect()
    }

    /// The note's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }

    /// The `AssertionId` of the current rich-text content, if set — the read side of the per-row
    /// correction the translations table drives (Edit supersedes it, Remove retracts it).
    #[must_use]
    pub fn text_assertion(&self) -> Option<crate::ids::AssertionId> {
        self.state.text.as_ref().map(|t| t.assertion_id)
    }

    /// The `AssertionId` of the current note type, if set.
    #[must_use]
    pub fn note_type_assertion(&self) -> Option<crate::ids::AssertionId> {
        self.state.note_type.as_ref().map(|t| t.assertion_id)
    }
}

impl View<NoteState> for NoteView {
    fn update(&mut self, event: &EventEnvelope<NoteState>) {
        evolve(&mut self.state, &event.payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assertions::Attributed;
    use crate::ids::AssertionId;
    use crate::text::MediaType;
    use uuid::Uuid;

    #[test]
    fn text_assertion_exposes_the_content_assertion() {
        let aid = AssertionId::from_uuid(Uuid::from_u128(7));
        let text = RichText {
            text: "hello".to_owned(),
            media_type: MediaType::Markdown,
            language: None,
            translator: None,
            translations: Vec::new(),
        };
        let state = NoteState {
            text: Some(Attributed {
                assertion_id: aid,
                value: text,
            }),
            ..Default::default()
        };
        let view = NoteView { state };
        assert_eq!(view.text_assertion(), Some(aid));
    }
}
