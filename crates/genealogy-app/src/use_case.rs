//! Shared use-case plumbing: command-outcome mapping and id resolution.
//!
//! The per-aggregate use-cases (`person`, `family`, …) all turn a store outcome into an
//! [`AppError`] and resolve a `human_id` to an aggregate id the same way; those two steps live here
//! so each use-case stays a thin, aggregate-specific wrapper.

use genealogy_db::CommandError;

use crate::error::AppError;

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
