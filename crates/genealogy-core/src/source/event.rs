//! Source events — the past-tense assertions the aggregate produces (data-model §10).

use serde::{Deserialize, Serialize};

use crate::assertions::{Envelope, EventBody};
use crate::ids::{AssertionId, HumanId, NoteId, SourceId, TagId};
use crate::repo_ref::RepoRef;
use crate::text::{Attribute, MediaRef};

/// A single Source assertion plus its provenance envelope (ADR 0004 §1).
pub type SourceEvent = Envelope<SourceEventBody>;

/// The Source claim variants (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SourceEventBody {
    /// A source aggregate was created.
    SourceCreated {
        /// The created source.
        source_id: SourceId,
        /// The user-facing identifier.
        human_id: HumanId,
    },
    /// The source's title was set / changed.
    TitleSet {
        /// The source.
        source_id: SourceId,
        /// The bibliographic title.
        title: String,
    },
    /// The source's author was set / changed.
    AuthorSet {
        /// The source.
        source_id: SourceId,
        /// The author.
        author: String,
    },
    /// The source's publication info was set / changed.
    PubInfoSet {
        /// The source.
        source_id: SourceId,
        /// The publication info.
        pub_info: String,
    },
    /// The source's abbreviation was set / changed.
    AbbrevSet {
        /// The source.
        source_id: SourceId,
        /// The abbreviation.
        abbrev: String,
    },
    /// The source was linked to a repository that holds it.
    RepositoryLinked {
        /// The source.
        source_id: SourceId,
        /// The repository link (call number + media type).
        repo_ref: RepoRef,
    },
    /// A typed attribute was added to the source.
    AttributeAdded {
        /// The source.
        source_id: SourceId,
        /// The attribute.
        attribute: Attribute,
    },
    /// A media reference was attached to the source.
    MediaAttached {
        /// The source.
        source_id: SourceId,
        /// The media reference.
        media: MediaRef,
    },
    /// A note was attached to the source.
    NoteAttached {
        /// The source.
        source_id: SourceId,
        /// The attached note.
        note_id: NoteId,
    },
    /// A tag was applied to the source.
    Tagged {
        /// The source.
        source_id: SourceId,
        /// The applied tag.
        tag_id: TagId,
    },
    /// A tag was removed from the source.
    Untagged {
        /// The source.
        source_id: SourceId,
        /// The removed tag.
        tag_id: TagId,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The source.
        source_id: SourceId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The source.
        source_id: SourceId,
        /// The assertion being superseded.
        target: AssertionId,
    },
}

impl EventBody for SourceEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::SourceCreated { .. } => "SourceCreated",
            Self::TitleSet { .. } => "TitleSet",
            Self::AuthorSet { .. } => "AuthorSet",
            Self::PubInfoSet { .. } => "PubInfoSet",
            Self::AbbrevSet { .. } => "AbbrevSet",
            Self::RepositoryLinked { .. } => "RepositoryLinked",
            Self::AttributeAdded { .. } => "AttributeAdded",
            Self::MediaAttached { .. } => "MediaAttached",
            Self::NoteAttached { .. } => "NoteAttached",
            Self::Tagged { .. } => "Tagged",
            Self::Untagged { .. } => "Untagged",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
        }
    }

    fn version(&self) -> &'static str {
        "1.0"
    }
}
