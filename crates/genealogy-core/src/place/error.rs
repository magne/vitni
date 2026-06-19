//! The Place domain-error taxonomy (data-model §10.1).
//!
//! Domain rejections (the rejection half of `decide`), never infrastructure failures. They become
//! the `cqrs-es` `Aggregate::Error`.

use thiserror::Error;

use crate::ids::PlaceId;

/// A reason the Place aggregate refused a command (data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum PlaceError {
    /// The command targets a place that does not exist.
    #[error("place {0} does not exist")]
    NotFound(PlaceId),
    /// `CreatePlace` was issued for a place that already exists.
    #[error("place {0} already exists")]
    AlreadyExists(PlaceId),
    /// A name was asserted with no text.
    #[error("a place name must not be empty")]
    EmptyName,
}
