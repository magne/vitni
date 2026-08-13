//! `ResearchNote` events — the past-tense assertions the aggregate produces (ADR 0028, data-model §10).

use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::{Envelope, EventBody};
use crate::enums::Restriction;
use crate::ids::{AssertionId, HumanId, ResearchNoteId, TagId};
use crate::research_note::subject::SubjectRef;
use crate::text::RichText;

/// A single `ResearchNote` assertion plus its provenance envelope (ADR 0004 §1).
pub type ResearchNoteEvent = Envelope<ResearchNoteEventBody>;

/// The `ResearchNote` claim variants (ADR 0028 §2, data-model §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum ResearchNoteEventBody {
    /// A research-note aggregate was created.
    ResearchNoteCreated {
        /// The created research note.
        research_note_id: ResearchNoteId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// The conclusion-bearing entities this argument is about (non-empty).
        subjects: BTreeSet<SubjectRef>,
        /// An optional short title.
        title: Option<String>,
    },
    /// A subject was added to the research note.
    SubjectAdded {
        /// The research note.
        research_note_id: ResearchNoteId,
        /// The added subject.
        subject: SubjectRef,
    },
    /// A subject was removed from the research note.
    SubjectRemoved {
        /// The research note.
        research_note_id: ResearchNoteId,
        /// The removed subject.
        subject: SubjectRef,
    },
    /// The note's written argument was set / changed.
    RichTextSet {
        /// The research note.
        research_note_id: ResearchNoteId,
        /// The argument text.
        body: RichText,
    },
    /// A tag was applied to the research note.
    Tagged {
        /// The research note.
        research_note_id: ResearchNoteId,
        /// The applied tag.
        tag_id: TagId,
    },
    /// A tag was removed from the research note.
    Untagged {
        /// The research note.
        research_note_id: ResearchNoteId,
        /// The removed tag.
        tag_id: TagId,
    },
    /// The research note's privacy restrictions were set / changed (GEDCOM `RESN` — data-model §6).
    RestrictionsChanged {
        /// The research note.
        research_note_id: ResearchNoteId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The research note.
        research_note_id: ResearchNoteId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The research note.
        research_note_id: ResearchNoteId,
        /// The assertion being superseded.
        target: AssertionId,
    },
}

impl EventBody for ResearchNoteEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::ResearchNoteCreated { .. } => "ResearchNoteCreated",
            Self::SubjectAdded { .. } => "SubjectAdded",
            Self::SubjectRemoved { .. } => "SubjectRemoved",
            Self::RichTextSet { .. } => "RichTextSet",
            Self::Tagged { .. } => "Tagged",
            Self::Untagged { .. } => "Untagged",
            Self::RestrictionsChanged { .. } => "RestrictionsChanged",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
        }
    }

    fn version(&self) -> &'static str {
        "1.0"
    }
}
