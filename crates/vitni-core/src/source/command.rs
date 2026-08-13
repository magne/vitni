//! Source commands — imperative operator intent (data-model §10).

use std::collections::BTreeSet;

use crate::enums::Restriction;
use crate::ids::{AssertionId, HumanId, NoteId, SourceId, TagId};
use crate::provenance::AssertionMeta;
use crate::repo_ref::RepoRef;
use crate::text::{Attribute, MediaRef};

/// Operator intent against a Source aggregate (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCommand {
    /// Create a new source.
    CreateSource {
        /// The application-generated id for the new source.
        source_id: SourceId,
        /// The user-facing identifier.
        human_id: HumanId,
    },
    /// Set (or change) the source's title.
    SetTitle {
        /// The target source.
        source_id: SourceId,
        /// The bibliographic title.
        title: String,
    },
    /// Set (or change) the source's author.
    SetAuthor {
        /// The target source.
        source_id: SourceId,
        /// The author.
        author: String,
    },
    /// Set (or change) the source's publication info.
    SetPubInfo {
        /// The target source.
        source_id: SourceId,
        /// The publication info.
        pub_info: String,
    },
    /// Set (or change) the source's abbreviation.
    SetAbbrev {
        /// The target source.
        source_id: SourceId,
        /// The abbreviation.
        abbrev: String,
    },
    /// Link the source to a repository that holds it (the cross-aggregate reference).
    LinkRepository {
        /// The target source.
        source_id: SourceId,
        /// The repository link (call number + media type).
        repo_ref: RepoRef,
    },
    /// Add a typed attribute to the source.
    AddAttribute {
        /// The target source.
        source_id: SourceId,
        /// The attribute.
        attribute: Attribute,
    },
    /// Attach a media reference to the source.
    AttachMedia {
        /// The target source.
        source_id: SourceId,
        /// The media reference.
        media: MediaRef,
    },
    /// Attach a note to the source.
    AttachNote {
        /// The target source.
        source_id: SourceId,
        /// The note to attach.
        note_id: NoteId,
    },
    /// Apply a tag to the source.
    Tag {
        /// The target source.
        source_id: SourceId,
        /// The tag to apply.
        tag_id: TagId,
    },
    /// Remove a tag from the source.
    Untag {
        /// The target source.
        source_id: SourceId,
        /// The tag to remove.
        tag_id: TagId,
    },
    /// Set (or change) the source's privacy restrictions (GEDCOM `RESN` — data-model §6).
    SetRestrictions {
        /// The target source.
        source_id: SourceId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// Retract a prior assertion (non-destructive).
    RetractAssertion {
        /// The target source.
        source_id: SourceId,
        /// The assertion to retract.
        target: AssertionId,
    },
    /// Supersede a prior assertion with a replacement command.
    SupersedeAssertion {
        /// The target source.
        source_id: SourceId,
        /// The assertion to supersede.
        target: AssertionId,
        /// The command producing the replacement assertion.
        replacement: Box<SourceCommand>,
    },
    /// Set (or change) the source's user-facing identifier (data-model §7).
    SetHumanId {
        /// The target source.
        source_id: SourceId,
        /// The new user-facing identifier.
        human_id: HumanId,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the Source aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: SourceCommand,
}
