//! Note commands — imperative operator intent (data-model §10).

use std::collections::BTreeSet;

use crate::enums::{NoteType, Restriction};
use crate::ids::{AssertionId, HumanId, NoteId, TagId};
use crate::provenance::AssertionMeta;
use crate::text::RichText;

/// Operator intent against a Note aggregate (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum NoteCommand {
    /// Create a new note.
    CreateNote {
        /// The application-generated id for the new note.
        note_id: NoteId,
        /// The user-facing identifier.
        human_id: HumanId,
    },
    /// Set (or change) the note's type.
    SetNoteType {
        /// The target note.
        note_id: NoteId,
        /// The new note type.
        note_type: NoteType,
    },
    /// Set (or change) the note's rich-text content.
    SetRichText {
        /// The target note.
        note_id: NoteId,
        /// The content.
        text: RichText,
    },
    /// Apply a tag to the note.
    Tag {
        /// The target note.
        note_id: NoteId,
        /// The tag to apply.
        tag_id: TagId,
    },
    /// Remove a tag from the note.
    Untag {
        /// The target note.
        note_id: NoteId,
        /// The tag to remove.
        tag_id: TagId,
    },
    /// Set (or change) the note's privacy restrictions (GEDCOM `RESN` — data-model §6).
    SetRestrictions {
        /// The target note.
        note_id: NoteId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// Retract a prior assertion (non-destructive).
    RetractAssertion {
        /// The target note.
        note_id: NoteId,
        /// The assertion to retract.
        target: AssertionId,
    },
    /// Supersede a prior assertion with a replacement command.
    SupersedeAssertion {
        /// The target note.
        note_id: NoteId,
        /// The assertion to supersede.
        target: AssertionId,
        /// The command producing the replacement assertion.
        replacement: Box<NoteCommand>,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the Note aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoteCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: NoteCommand,
}
