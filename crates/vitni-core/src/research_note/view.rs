//! [`ResearchNoteView`] — the conclusion-layer read model for a `ResearchNote` (ADR 0028).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::enums::Restriction;
use crate::ids::{HumanId, ResearchNoteId, TagId};
use crate::research_note::decide::evolve;
use crate::research_note::state::ResearchNoteState;
use crate::research_note::subject::SubjectRef;
use crate::text::RichText;

/// The current best synthesis of a `ResearchNote`, derived from the event log (ADR 0028).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResearchNoteView {
    state: ResearchNoteState,
}

impl ResearchNoteView {
    /// Returns `true` once the research note has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The research note's id, once created.
    #[must_use]
    pub fn research_note_id(&self) -> Option<ResearchNoteId> {
        self.state.research_note_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// The conclusion-bearing entities this argument is about (non-empty once created).
    #[must_use]
    pub fn subjects(&self) -> &BTreeSet<SubjectRef> {
        &self.state.subjects
    }

    /// The optional short title.
    #[must_use]
    pub fn title(&self) -> Option<&str> {
        self.state.title.as_deref()
    }

    /// The note's written argument, if set.
    #[must_use]
    pub fn body(&self) -> Option<&RichText> {
        self.state.body.as_ref().map(|b| &b.value)
    }

    /// All currently-applied tags, in assertion order.
    #[must_use]
    pub fn tags(&self) -> Vec<TagId> {
        self.state.tags.iter().map(|t| t.value).collect()
    }

    /// The research note's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }

    /// The `AssertionId` of the current body, if set — the read side of the per-row correction
    /// (Edit supersedes it, Remove retracts it).
    #[must_use]
    pub fn body_assertion(&self) -> Option<crate::ids::AssertionId> {
        self.state.body.as_ref().map(|b| b.assertion_id)
    }
}

impl View<ResearchNoteState> for ResearchNoteView {
    fn update(&mut self, event: &EventEnvelope<ResearchNoteState>) {
        evolve(&mut self.state, &event.payload);
    }
}

#[cfg(test)]
mod tests {
    use super::{ResearchNoteState, ResearchNoteView};
    use crate::assertions::Attributed;
    use crate::ids::AssertionId;
    use crate::text::{MediaType, RichText};
    use uuid::Uuid;

    #[test]
    fn body_assertion_exposes_the_content_assertion() {
        let aid = AssertionId::from_uuid(Uuid::from_u128(7));
        let body = RichText {
            text: "hello".to_owned(),
            media_type: MediaType::Markdown,
            language: None,
            translator: None,
            translations: Vec::new(),
        };
        let state = ResearchNoteState {
            body: Some(Attributed {
                assertion_id: aid,
                value: body,
            }),
            ..Default::default()
        };
        let view = ResearchNoteView { state };
        assert_eq!(view.body_assertion(), Some(aid));
    }
}
