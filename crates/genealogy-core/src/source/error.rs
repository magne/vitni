//! The Source domain-error taxonomy (data-model §10.1).

use thiserror::Error;

use crate::ids::SourceId;

/// A reason the Source aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum SourceError {
    /// The command targets a source that does not exist.
    #[error("source {0} does not exist")]
    NotFound(SourceId),
    /// `CreateSource` was issued for a source that already exists.
    #[error("source {0} already exists")]
    AlreadyExists(SourceId),
}
