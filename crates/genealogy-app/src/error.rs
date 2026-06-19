//! [`AppError`] — the single error type the use-cases return to a frontend.
//!
//! It collapses configuration, workspace I/O, the engine-neutral store ([`genealogy_db::DbError`]),
//! and domain rejections ([`PersonError`]) into one enum so a frontend can render a message and pick
//! an exit status without knowing which layer failed. Domain rejections are kept distinct
//! ([`AppError::Domain`]) because they are the operator's fault (a 4xx), not the system's.

use genealogy_core::citation::CitationError;
use genealogy_core::event::EventError;
use genealogy_core::family::FamilyError;
use genealogy_core::person::PersonError;
use genealogy_core::place::PlaceError;
use genealogy_core::source::SourceError;
use genealogy_db::DbError;

/// A failure surfaced by a `genealogy-app` use-case.
#[derive(Debug, thiserror::Error)]
pub enum AppError {
    /// Configuration could not be loaded, bootstrapped, written, or parsed.
    #[error("configuration error: {0}")]
    Config(String),
    /// A workspace directory or its manifest could not be created/read.
    #[error("workspace error: {0}")]
    Workspace(String),
    /// The engine-neutral store failed (infrastructure).
    #[error(transparent)]
    Db(#[from] DbError),
    /// A caller-supplied `human_id` is already in use.
    #[error("human_id {0:?} is already taken")]
    HumanIdTaken(String),
    /// No person exists with the given `human_id`.
    #[error("no person with human_id {0:?}")]
    PersonNotFound(String),
    /// No family exists with the given `human_id`.
    #[error("no family with human_id {0:?}")]
    FamilyNotFound(String),
    /// No place exists with the given `human_id`.
    #[error("no place with human_id {0:?}")]
    PlaceNotFound(String),
    /// No source exists with the given `human_id`.
    #[error("no source with human_id {0:?}")]
    SourceNotFound(String),
    /// No citation exists with the given `human_id`.
    #[error("no citation with human_id {0:?}")]
    CitationNotFound(String),
    /// No event exists with the given `human_id`.
    #[error("no event with human_id {0:?}")]
    EventNotFound(String),
    /// The command was rejected by a Person domain rule (the operator's input is invalid).
    #[error("rejected: {0}")]
    Domain(PersonError),
    /// The command was rejected by a Family domain rule (the operator's input is invalid).
    #[error("rejected: {0}")]
    FamilyDomain(FamilyError),
    /// The command was rejected by a Place domain rule (the operator's input is invalid).
    #[error("rejected: {0}")]
    PlaceDomain(PlaceError),
    /// The command was rejected by a Source domain rule (the operator's input is invalid).
    #[error("rejected: {0}")]
    SourceDomain(SourceError),
    /// The command was rejected by a Citation domain rule (the operator's input is invalid).
    #[error("rejected: {0}")]
    CitationDomain(CitationError),
    /// The command was rejected by an Event domain rule (the operator's input is invalid).
    #[error("rejected: {0}")]
    EventDomain(EventError),
}
