//! The Tag domain-error taxonomy (data-model §10.1).

use thiserror::Error;

use crate::ids::TagId;

/// A reason the Tag aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TagError {
    /// The command targets a tag that does not exist.
    #[error("tag {0} does not exist")]
    NotFound(TagId),
    /// `CreateTag` was issued for a tag that already exists.
    #[error("tag {0} already exists")]
    AlreadyExists(TagId),
    /// A name was set with no text.
    #[error("a tag name must not be empty")]
    EmptyName,
}
