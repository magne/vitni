//! `ResearchNote` commands — imperative operator intent (ADR 0028, data-model §10).

use std::collections::BTreeSet;

use crate::enums::Restriction;
use crate::ids::{AssertionId, HumanId, ResearchNoteId, TagId};
use crate::provenance::AssertionMeta;
use crate::research_note::subject::SubjectRef;
use crate::text::RichText;

/// Operator intent against a `ResearchNote` aggregate (ADR 0028 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ResearchNoteCommand {
    /// Create a new research note arguing about `subjects` (at least one).
    CreateResearchNote {
        /// The application-generated id for the new research note.
        research_note_id: ResearchNoteId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// The conclusion-bearing entities this argument is about — must be non-empty.
        subjects: BTreeSet<SubjectRef>,
        /// An optional short title.
        title: Option<String>,
    },
    /// Add another subject to an existing research note (idempotent if already named).
    AddSubject {
        /// The target research note.
        research_note_id: ResearchNoteId,
        /// The subject to add.
        subject: SubjectRef,
    },
    /// Remove a subject from an existing research note. Rejected if `subject` is the note's only
    /// remaining one (a `ResearchNote` always names at least one subject — ADR 0028 §2).
    RemoveSubject {
        /// The target research note.
        research_note_id: ResearchNoteId,
        /// The subject to remove.
        subject: SubjectRef,
    },
    /// Set (or change) the note's written argument.
    SetBody {
        /// The target research note.
        research_note_id: ResearchNoteId,
        /// The argument text.
        body: RichText,
    },
    /// Apply a tag to the research note.
    Tag {
        /// The target research note.
        research_note_id: ResearchNoteId,
        /// The tag to apply.
        tag_id: TagId,
    },
    /// Remove a tag from the research note.
    Untag {
        /// The target research note.
        research_note_id: ResearchNoteId,
        /// The tag to remove.
        tag_id: TagId,
    },
    /// Set (or change) the research note's privacy restrictions (GEDCOM `RESN` — data-model §6).
    SetRestrictions {
        /// The target research note.
        research_note_id: ResearchNoteId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// Retract a prior assertion (non-destructive).
    RetractAssertion {
        /// The target research note.
        research_note_id: ResearchNoteId,
        /// The assertion to retract.
        target: AssertionId,
    },
    /// Supersede a prior assertion with a replacement command.
    SupersedeAssertion {
        /// The target research note.
        research_note_id: ResearchNoteId,
        /// The assertion to supersede.
        target: AssertionId,
        /// The command producing the replacement assertion.
        replacement: Box<ResearchNoteCommand>,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the `ResearchNote` aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResearchNoteCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: ResearchNoteCommand,
}
