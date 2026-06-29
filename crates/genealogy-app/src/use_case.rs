//! Shared use-case plumbing: command-outcome mapping and id resolution.
//!
//! The per-aggregate use-cases (`person`, `family`, …) all turn a store outcome into an
//! [`AppError`] and resolve a `human_id` to an aggregate id the same way; those two steps live here
//! so each use-case stays a thin, aggregate-specific wrapper.

use std::collections::HashMap;

use genealogy_core::ids::{MediaId, NoteId};
use genealogy_core::provenance::Confidence;
use genealogy_db::{CommandError, Store};

use crate::error::AppError;

/// The operator's surety in, and reason for, a single assertion — the per-assertion provenance the
/// frontend supplies (data-model §8). Defaults to [`Confidence::Normal`] with no rationale, so a
/// caller that does not collect it keeps the previous behavior.
#[derive(Debug, Clone)]
pub struct Provenance {
    /// The operator's surety in this claim.
    pub confidence: Confidence,
    /// Why the claim was made (free text; GENTECH rationale / GEDCOM X change message).
    pub rationale: Option<String>,
}

impl Default for Provenance {
    fn default() -> Self {
        Self {
            confidence: Confidence::Normal,
            rationale: None,
        }
    }
}

/// Maps a [`CommandError`] to [`AppError`]: a domain rejection becomes the matching `…Domain`
/// variant (via its `From` impl), a store failure becomes [`AppError::Db`]. This keeps the
/// operator's fault (a 4xx) distinct from the system's.
pub(crate) fn map_command_error<E>(error: CommandError<E>) -> AppError
where
    E: std::error::Error,
    AppError: From<E>,
{
    match error {
        CommandError::Rejected(domain) => AppError::from(domain),
        CommandError::Store(db) => AppError::Db(db),
    }
}

/// Loads a `MediaId -> human_id` lookup from the Media projection.
pub(crate) async fn media_human_ids(store: &Store) -> Result<HashMap<MediaId, String>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_media().await? {
        if let (Some(id), Some(human_id)) = (view.media_id(), view.human_id()) {
            map.insert(id, human_id.as_str().to_owned());
        }
    }
    Ok(map)
}

/// Loads a `NoteId -> human_id` lookup from the Note projection.
pub(crate) async fn note_human_ids(store: &Store) -> Result<HashMap<NoteId, String>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_notes().await? {
        if let (Some(id), Some(human_id)) = (view.note_id(), view.human_id()) {
            map.insert(id, human_id.as_str().to_owned());
        }
    }
    Ok(map)
}

/// Resolves a looked-up view to an aggregate id: `extract` pulls the id from the view, and a missing
/// view (or a view without the id) becomes the `not_found` error. Centralizes the
/// find → extract → not-found pattern every `resolve_<agg>_id` helper repeats.
pub(crate) fn resolve_id<V, Id>(
    found: Option<V>,
    extract: impl FnOnce(&V) -> Option<Id>,
    not_found: impl FnOnce() -> AppError,
) -> Result<Id, AppError> {
    let Some(view) = found else {
        return Err(not_found());
    };
    extract(&view).ok_or_else(not_found)
}
