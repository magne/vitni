//! Media commands — imperative operator intent (data-model §10).

use std::collections::BTreeSet;

use crate::date::GenealogicalDate;
use crate::enums::Restriction;
use crate::ids::{AssertionId, CitationId, HumanId, MediaId, NoteId, TagId};
use crate::media_path::MediaPath;
use crate::provenance::AssertionMeta;
use crate::text::Attribute;

/// Operator intent against a Media aggregate (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MediaCommand {
    /// Create a new media object.
    CreateMedia {
        /// The application-generated id for the new media.
        media_id: MediaId,
        /// The user-facing identifier.
        human_id: HumanId,
    },
    /// Set (or change) the media's location (a file path or web reference).
    SetPath {
        /// The target media.
        media_id: MediaId,
        /// The location.
        path: MediaPath,
    },
    /// Set (or change) the media's checksum.
    SetChecksum {
        /// The target media.
        media_id: MediaId,
        /// The checksum.
        checksum: String,
    },
    /// Set (or change) the media's MIME type.
    SetMime {
        /// The target media.
        media_id: MediaId,
        /// The MIME type (e.g. `image/jpeg`).
        mime: String,
    },
    /// Assert the date of the media artifact.
    AssertDate {
        /// The target media.
        media_id: MediaId,
        /// The date.
        date: GenealogicalDate,
    },
    /// Add a typed attribute to the media.
    AddAttribute {
        /// The target media.
        media_id: MediaId,
        /// The attribute.
        attribute: Attribute,
    },
    /// Add a citation backing the media's claims.
    AddCitation {
        /// The target media.
        media_id: MediaId,
        /// The citation to add.
        citation_id: CitationId,
    },
    /// Attach a note to the media.
    AttachNote {
        /// The target media.
        media_id: MediaId,
        /// The note to attach.
        note_id: NoteId,
    },
    /// Apply a tag to the media.
    Tag {
        /// The target media.
        media_id: MediaId,
        /// The tag to apply.
        tag_id: TagId,
    },
    /// Remove a tag from the media.
    Untag {
        /// The target media.
        media_id: MediaId,
        /// The tag to remove.
        tag_id: TagId,
    },
    /// Set (or change) the media's privacy restrictions (GEDCOM `RESN` — data-model §6).
    SetRestrictions {
        /// The target media.
        media_id: MediaId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// Retract a prior assertion (non-destructive).
    RetractAssertion {
        /// The target media.
        media_id: MediaId,
        /// The assertion to retract.
        target: AssertionId,
    },
    /// Supersede a prior assertion with a replacement command.
    SupersedeAssertion {
        /// The target media.
        media_id: MediaId,
        /// The assertion to supersede.
        target: AssertionId,
        /// The command producing the replacement assertion.
        replacement: Box<MediaCommand>,
    },
    /// Set (or change) the media's user-facing identifier (data-model §7).
    SetHumanId {
        /// The target media.
        media_id: MediaId,
        /// The new user-facing identifier.
        human_id: HumanId,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the Media aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: MediaCommand,
}
