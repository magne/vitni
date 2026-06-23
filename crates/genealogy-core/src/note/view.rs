//! [`NoteView`] — the conclusion-layer read model for a Note (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::enums::{NoteType, Restriction};
use crate::ids::{HumanId, NoteId};
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

    /// The note's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }
}

impl View<NoteState> for NoteView {
    fn update(&mut self, event: &EventEnvelope<NoteState>) {
        evolve(&mut self.state, &event.payload);
    }
}
