//! Media use-cases (ADR 0006): create, set path/checksum, assert date, add attribute/citation,
//! attach note, tag, show, and list.

use genealogy_core::citation::CitationView;
use genealogy_core::ids::{CitationId, HumanId, MediaId, NoteId, TagId};
use genealogy_core::media::MediaView;
use genealogy_core::media::command::{MediaCommand, MediaCommandEnvelope};
use genealogy_core::media_path::MediaPath;
use genealogy_core::provenance::Confidence;
use genealogy_core::text::{Attribute, Url};
use genealogy_db::Store;

use crate::error::AppError;
use crate::event::{DateParts, gregorian_date};
use crate::session::Session;
use crate::use_case;
use crate::workspace::Workspace;

/// A frontend-neutral summary of a media object (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaSummary {
    /// The user-facing identifier (e.g. `O0001`).
    pub human_id: String,
    /// The media's location rendered for display, if set.
    pub path: Option<String>,
    /// The media's checksum, if set.
    pub checksum: Option<String>,
    /// The number of recorded attributes.
    pub attribute_count: usize,
}

/// What to create a media object with (the auto/override `human_id` and an optional file path).
#[derive(Debug, Clone)]
pub struct NewMedia {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// An optional file path for an initial `SetPath`.
    pub path: Option<String>,
}

/// Creates a media object, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::MediaDomain`] if a domain rule
/// rejects the command, or a workspace/store error.
pub async fn create_media(workspace: &Workspace, session: &Session, new: NewMedia) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_media(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_media_human_id(&workspace.media_id_format()?).await?,
    };

    let media_id = session.new_media_id();
    let aggregate_id = media_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        MediaCommand::CreateMedia {
            media_id,
            human_id: HumanId::new(&human_id),
        },
    )
    .await?;

    if let Some(path) = new.path {
        execute(
            store,
            session,
            &aggregate_id,
            MediaCommand::SetPath {
                media_id,
                path: MediaPath::File(path),
            },
        )
        .await?;
    }

    Ok(human_id)
}

/// Sets (or changes) a media object's file path, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn set_media_file_path(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    path: String,
) -> Result<(), AppError> {
    set_media_path(workspace, session, human_id, MediaPath::File(path)).await
}

/// Sets (or changes) a media object's web reference, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn set_media_web_path(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    href: String,
) -> Result<(), AppError> {
    let path = MediaPath::Web(Url {
        url_type: None,
        href,
        description: None,
    });
    set_media_path(workspace, session, human_id, path).await
}

/// Sets (or changes) a media object's checksum, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn set_media_checksum(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    checksum: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::SetChecksum { media_id, checksum },
    )
    .await
}

/// Asserts a media object's date, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn assert_media_date(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    parts: DateParts,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::AssertDate {
            media_id,
            date: gregorian_date(parts),
        },
    )
    .await
}

/// Adds a typed attribute to a media object, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn add_media_attribute(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    attribute_type: String,
    value: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::AddAttribute {
            media_id,
            attribute: Attribute {
                attribute_type,
                value,
                citations: Vec::new(),
            },
        },
    )
    .await
}

/// Adds a citation (by its `human_id`) backing a media object's claims.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] / [`AppError::CitationNotFound`] if either is unknown, or a
/// workspace/store error.
pub async fn add_media_citation(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    citation_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    let citation_id = resolve_citation_id(store, citation_human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::AddCitation { media_id, citation_id },
    )
    .await
}

/// Attaches a note (by note aggregate id) to a media object, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn attach_media_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_id: NoteId,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::AttachNote { media_id, note_id },
    )
    .await
}

/// Applies (or removes) a tag on a media object, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::MediaNotFound`] if no such media exists, or a workspace/store error.
pub async fn tag_media(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: TagId,
    remove: bool,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    let command = if remove {
        MediaCommand::Untag { media_id, tag_id }
    } else {
        MediaCommand::Tag { media_id, tag_id }
    };
    execute(store, session, &media_id.to_string(), command).await
}

/// Loads a single media object's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_media(workspace: &Workspace, human_id: &str) -> Result<Option<MediaSummary>, AppError> {
    let found = workspace.store().find_media(human_id).await?;
    Ok(found.as_ref().map(summarize))
}

/// Lists every media object's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_media(workspace: &Workspace) -> Result<Vec<MediaSummary>, AppError> {
    let views = workspace.store().list_media().await?;
    let mut summaries = Vec::with_capacity(views.len());
    for view in &views {
        summaries.push(summarize(view));
    }
    Ok(summaries)
}

/// Sets the media path through the store.
async fn set_media_path(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    path: MediaPath,
) -> Result<(), AppError> {
    let store = workspace.store();
    let media_id = resolve_media_id(store, human_id).await?;
    execute(
        store,
        session,
        &media_id.to_string(),
        MediaCommand::SetPath { media_id, path },
    )
    .await
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
async fn execute(store: &Store, session: &Session, aggregate_id: &str, command: MediaCommand) -> Result<(), AppError> {
    let envelope = MediaCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_media(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Resolves a `human_id` to its aggregate [`MediaId`], or [`AppError::MediaNotFound`].
async fn resolve_media_id(store: &Store, human_id: &str) -> Result<MediaId, AppError> {
    use_case::resolve_id(store.find_media(human_id).await?, MediaView::media_id, || {
        AppError::MediaNotFound(human_id.to_owned())
    })
}

/// Resolves a citation `human_id` to its aggregate [`CitationId`], or [`AppError::CitationNotFound`].
async fn resolve_citation_id(store: &Store, human_id: &str) -> Result<CitationId, AppError> {
    use_case::resolve_id(store.find_citation(human_id).await?, CitationView::citation_id, || {
        AppError::CitationNotFound(human_id.to_owned())
    })
}

/// Renders the media location for display.
fn render_path(path: &MediaPath) -> String {
    match path {
        MediaPath::File(file) => file.clone(),
        MediaPath::Web(url) => url.href.clone(),
    }
}

/// Renders a [`MediaView`] into the frontend DTO.
fn summarize(view: &MediaView) -> MediaSummary {
    MediaSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        path: view.path().map(render_path),
        checksum: view.checksum().map(ToOwned::to_owned),
        attribute_count: view.attributes().len(),
    }
}
