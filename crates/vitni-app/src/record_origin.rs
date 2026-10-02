//! A record's import origin (ADR 0037 §4): the dataset record an import created it from, which the
//! compare view shows as an origin chip beside each side of a pair (ADR 0038 §3).

use uuid::Uuid;
use vitni_core::citation::CitationView;
use vitni_core::event::EventView;
use vitni_core::family::FamilyView;
use vitni_core::ids::TagId;
use vitni_core::matching::MatchableKind;
use vitni_core::media::MediaView;
use vitni_core::note::NoteView;
use vitni_core::origin::RecordOrigin;
use vitni_core::person::PersonView;
use vitni_core::place::PlaceView;
use vitni_core::repository::RepositoryView;
use vitni_core::source::SourceView;

use crate::error::AppError;
use crate::use_case::resolve_id;
use crate::workspace::Workspace;

/// The origin of the import that created the record `human_id` of `kind` (a tag's id), or `None` when
/// it was entered by hand. The origin carries no content digest.
///
/// # Errors
///
/// The kind's `NotFound` error if no such record exists, or [`AppError`] on a store read failure.
pub async fn record_origin(
    workspace: &Workspace,
    kind: MatchableKind,
    human_id: &str,
) -> Result<Option<RecordOrigin>, AppError> {
    let aggregate_id = aggregate_id(workspace, kind, human_id).await?;
    Ok(workspace.store().created_origin(kind.as_str(), &aggregate_id).await?)
}

/// The aggregate id of the record `human_id` of `kind`.
async fn aggregate_id(workspace: &Workspace, kind: MatchableKind, human_id: &str) -> Result<String, AppError> {
    let store = workspace.store();
    let id = human_id.to_owned();
    let id = match kind {
        MatchableKind::Person => resolve_id(store.find_person(human_id).await?, PersonView::person_id, || {
            AppError::PersonNotFound(id)
        })?
        .to_string(),
        MatchableKind::Family => resolve_id(store.find_family(human_id).await?, FamilyView::family_id, || {
            AppError::FamilyNotFound(id)
        })?
        .to_string(),
        MatchableKind::Event => resolve_id(store.find_event(human_id).await?, EventView::event_id, || {
            AppError::EventNotFound(id)
        })?
        .to_string(),
        MatchableKind::Place => resolve_id(store.find_place(human_id).await?, PlaceView::place_id, || {
            AppError::PlaceNotFound(id)
        })?
        .to_string(),
        MatchableKind::Source => resolve_id(store.find_source(human_id).await?, SourceView::source_id, || {
            AppError::SourceNotFound(id)
        })?
        .to_string(),
        MatchableKind::Repository => resolve_id(
            store.find_repository(human_id).await?,
            RepositoryView::repository_id,
            || AppError::RepositoryNotFound(id),
        )?
        .to_string(),
        MatchableKind::Citation => resolve_id(store.find_citation(human_id).await?, CitationView::citation_id, || {
            AppError::CitationNotFound(id)
        })?
        .to_string(),
        MatchableKind::Media => resolve_id(store.find_media(human_id).await?, MediaView::media_id, || {
            AppError::MediaNotFound(id)
        })?
        .to_string(),
        MatchableKind::Note => resolve_id(store.find_note(human_id).await?, NoteView::note_id, || {
            AppError::NoteNotFound(id)
        })?
        .to_string(),
        MatchableKind::Tag => Uuid::parse_str(human_id)
            .map(TagId::from_uuid)
            .map_err(|_| AppError::TagNotFound(id))?
            .to_string(),
    };
    Ok(id)
}
