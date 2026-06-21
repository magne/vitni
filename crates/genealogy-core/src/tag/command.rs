//! Tag commands — imperative operator intent (data-model §10).
//!
//! A `Tag` is the lightweight *definition* (name/colour/priority); applying a tag is an event on the
//! tagged aggregate, not here (data-model §9). Tags carry no `HumanId` and no assertion corrections:
//! the setters are last-writer-wins.

use crate::ids::TagId;
use crate::provenance::AssertionMeta;

/// Operator intent against a Tag aggregate (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TagCommand {
    /// Create a new tag with a name.
    CreateTag {
        /// The application-generated id for the new tag.
        tag_id: TagId,
        /// The tag's name.
        name: String,
    },
    /// Rename the tag.
    RenameTag {
        /// The target tag.
        tag_id: TagId,
        /// The new name.
        name: String,
    },
    /// Set (or change) the tag's colour.
    SetTagColor {
        /// The target tag.
        tag_id: TagId,
        /// The colour (e.g. a hex string like `#1f77b4`).
        color: String,
    },
    /// Set (or change) the tag's sort priority.
    SetTagPriority {
        /// The target tag.
        tag_id: TagId,
        /// The priority (lower sorts first).
        priority: i32,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the Tag aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: TagCommand,
}
