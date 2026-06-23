//! Tag events — the past-tense assertions the aggregate produces (data-model §10).

use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::{Envelope, EventBody};
use crate::enums::Restriction;
use crate::ids::TagId;

/// A single Tag assertion plus its provenance envelope (ADR 0004 §1).
pub type TagEvent = Envelope<TagEventBody>;

/// The Tag claim variants (data-model §10). Setters are last-writer-wins; there is no
/// retract/supersede pair (a tag definition has no assertion chain to correct).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum TagEventBody {
    /// A tag aggregate was created.
    TagCreated {
        /// The created tag.
        tag_id: TagId,
        /// The tag's name.
        name: String,
    },
    /// The tag was renamed.
    TagRenamed {
        /// The tag.
        tag_id: TagId,
        /// The new name.
        name: String,
    },
    /// The tag's colour was set / changed.
    TagColorSet {
        /// The tag.
        tag_id: TagId,
        /// The colour.
        color: String,
    },
    /// The tag's priority was set / changed.
    TagPrioritySet {
        /// The tag.
        tag_id: TagId,
        /// The priority.
        priority: i32,
    },
    /// The tag's privacy restrictions were set / changed (GEDCOM `RESN` — data-model §6).
    RestrictionsChanged {
        /// The tag.
        tag_id: TagId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
}

impl EventBody for TagEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::TagCreated { .. } => "TagCreated",
            Self::TagRenamed { .. } => "TagRenamed",
            Self::TagColorSet { .. } => "TagColorSet",
            Self::TagPrioritySet { .. } => "TagPrioritySet",
            Self::RestrictionsChanged { .. } => "RestrictionsChanged",
        }
    }

    fn version(&self) -> &'static str {
        "1.0"
    }
}
