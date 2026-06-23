//! Media events — the past-tense assertions the aggregate produces (data-model §10).

use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::{Envelope, EventBody};
use crate::date::GenealogicalDate;
use crate::enums::Restriction;
use crate::ids::{AssertionId, CitationId, HumanId, MediaId, NoteId, TagId};
use crate::media_path::MediaPath;
use crate::text::Attribute;

/// A single Media assertion plus its provenance envelope (ADR 0004 §1).
pub type MediaEvent = Envelope<MediaEventBody>;

/// The Media claim variants (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum MediaEventBody {
    /// A media aggregate was created.
    MediaCreated {
        /// The created media.
        media_id: MediaId,
        /// The user-facing identifier.
        human_id: HumanId,
    },
    /// The media's location was set / changed.
    PathSet {
        /// The media.
        media_id: MediaId,
        /// The location.
        path: MediaPath,
    },
    /// The media's checksum was set / changed.
    ChecksumSet {
        /// The media.
        media_id: MediaId,
        /// The checksum.
        checksum: String,
    },
    /// The media's date was asserted.
    DateAsserted {
        /// The media.
        media_id: MediaId,
        /// The date.
        date: GenealogicalDate,
    },
    /// A typed attribute was added to the media.
    AttributeAdded {
        /// The media.
        media_id: MediaId,
        /// The attribute.
        attribute: Attribute,
    },
    /// A citation was added to the media.
    CitationAdded {
        /// The media.
        media_id: MediaId,
        /// The added citation.
        citation_id: CitationId,
    },
    /// A note was attached to the media.
    NoteAttached {
        /// The media.
        media_id: MediaId,
        /// The attached note.
        note_id: NoteId,
    },
    /// A tag was applied to the media.
    Tagged {
        /// The media.
        media_id: MediaId,
        /// The applied tag.
        tag_id: TagId,
    },
    /// A tag was removed from the media.
    Untagged {
        /// The media.
        media_id: MediaId,
        /// The removed tag.
        tag_id: TagId,
    },
    /// The media's privacy restrictions were set / changed (GEDCOM `RESN` — data-model §6).
    RestrictionsChanged {
        /// The media.
        media_id: MediaId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The media.
        media_id: MediaId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The media.
        media_id: MediaId,
        /// The assertion being superseded.
        target: AssertionId,
    },
}

impl EventBody for MediaEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::MediaCreated { .. } => "MediaCreated",
            Self::PathSet { .. } => "PathSet",
            Self::ChecksumSet { .. } => "ChecksumSet",
            Self::DateAsserted { .. } => "DateAsserted",
            Self::AttributeAdded { .. } => "AttributeAdded",
            Self::CitationAdded { .. } => "CitationAdded",
            Self::NoteAttached { .. } => "NoteAttached",
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
