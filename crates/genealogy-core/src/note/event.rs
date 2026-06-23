//! Note events — the past-tense assertions the aggregate produces (data-model §10).

use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::{Envelope, EventBody};
use crate::enums::{NoteType, Restriction};
use crate::ids::{AssertionId, HumanId, NoteId, TagId};
use crate::text::RichText;

/// A single Note assertion plus its provenance envelope (ADR 0004 §1).
pub type NoteEvent = Envelope<NoteEventBody>;

/// The Note claim variants (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum NoteEventBody {
    /// A note aggregate was created.
    NoteCreated {
        /// The created note.
        note_id: NoteId,
        /// The user-facing identifier.
        human_id: HumanId,
    },
    /// The note's type was set / changed.
    NoteTypeSet {
        /// The note.
        note_id: NoteId,
        /// The new note type.
        note_type: NoteType,
    },
    /// The note's rich-text content was set / changed.
    RichTextSet {
        /// The note.
        note_id: NoteId,
        /// The content.
        text: RichText,
    },
    /// A tag was applied to the note.
    Tagged {
        /// The note.
        note_id: NoteId,
        /// The applied tag.
        tag_id: TagId,
    },
    /// A tag was removed from the note.
    Untagged {
        /// The note.
        note_id: NoteId,
        /// The removed tag.
        tag_id: TagId,
    },
    /// The note's privacy restrictions were set / changed (GEDCOM `RESN` — data-model §6).
    RestrictionsChanged {
        /// The note.
        note_id: NoteId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The note.
        note_id: NoteId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The note.
        note_id: NoteId,
        /// The assertion being superseded.
        target: AssertionId,
    },
}

impl EventBody for NoteEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::NoteCreated { .. } => "NoteCreated",
            Self::NoteTypeSet { .. } => "NoteTypeSet",
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
