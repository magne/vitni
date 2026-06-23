//! Source use-cases (ADR 0006): create, set title, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`SourceSummary`]. `human_id` is auto-allocated using the workspace's configured
//! format, or validated when supplied (ADR 0005).

use std::collections::BTreeSet;

use genealogy_core::enums::{Restriction, SourceMediaType};
use genealogy_core::ids::{HumanId, MediaId, NoteId, RepositoryId, SourceId, TagId};
use genealogy_core::provenance::Confidence;
use genealogy_core::repo_ref::RepoRef;
use genealogy_core::repository::RepositoryView;
use genealogy_core::source::SourceView;
use genealogy_core::source::command::{SourceCommand, SourceCommandEnvelope};
use genealogy_core::text::{Attribute, MediaRef};
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case;
use crate::workspace::Workspace;

/// A frontend-neutral summary of a source (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSummary {
    /// The user-facing identifier (e.g. `S0001`).
    pub human_id: String,
    /// The bibliographic title, if set.
    pub title: Option<String>,
    /// The author, if set.
    pub author: Option<String>,
    /// The publication info, if set.
    pub pub_info: Option<String>,
    /// The abbreviation, if set.
    pub abbrev: Option<String>,
    /// The linked repositories (their aggregate ids), in assertion order.
    pub repositories: Vec<String>,
    /// The source's attributes rendered as `type=value`, in assertion order.
    pub attributes: Vec<String>,
    /// The source's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// What to create a source with (the auto/override `human_id` and an optional title).
#[derive(Debug, Clone)]
pub struct NewSource {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// An optional title for an initial `SetTitle`.
    pub title: Option<String>,
}

/// Creates a source, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::SourceDomain`] if a domain
/// rule rejects the command, or a workspace/store error.
pub async fn create_source(workspace: &Workspace, session: &Session, new: NewSource) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_source(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_source_human_id(&workspace.source_id_format()?).await?,
    };

    let source_id = session.new_source_id();
    let aggregate_id = source_id.to_string();

    execute(
        store,
        session,
        &aggregate_id,
        SourceCommand::CreateSource {
            source_id,
            human_id: HumanId::new(&human_id),
        },
    )
    .await?;

    if let Some(title) = new.title {
        execute(
            store,
            session,
            &aggregate_id,
            SourceCommand::SetTitle { source_id, title },
        )
        .await?;
    }

    Ok(human_id)
}

/// Sets (or changes) an existing source's title, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn set_title(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    title: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute(
        store,
        session,
        &source_id.to_string(),
        SourceCommand::SetTitle { source_id, title },
    )
    .await
}

/// Sets (or changes) an existing source's author, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn set_source_author(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    author: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute(
        store,
        session,
        &source_id.to_string(),
        SourceCommand::SetAuthor { source_id, author },
    )
    .await
}

/// Sets (or changes) an existing source's publication info, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn set_source_pub_info(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    pub_info: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute(
        store,
        session,
        &source_id.to_string(),
        SourceCommand::SetPubInfo { source_id, pub_info },
    )
    .await
}

/// Sets (or changes) an existing source's abbreviation, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn set_source_abbrev(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    abbrev: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute(
        store,
        session,
        &source_id.to_string(),
        SourceCommand::SetAbbrev { source_id, abbrev },
    )
    .await
}

/// Links a source to a repository (by its `human_id`) that holds it.
///
/// # Errors
///
/// [`AppError::SourceNotFound`]/[`AppError::RepositoryNotFound`] if either is unknown,
/// [`AppError::SourceDomain`] if the repository is not yet projected (`UnknownRepository`), or a
/// workspace/store error.
pub async fn link_source_repository(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    repository_human_id: &str,
    call_number: Option<String>,
    media_type: SourceMediaType,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    let repository_id = resolve_repository_id(store, repository_human_id).await?;
    execute(
        store,
        session,
        &source_id.to_string(),
        SourceCommand::LinkRepository {
            source_id,
            repo_ref: RepoRef {
                repository_id,
                call_number,
                media_type,
            },
        },
    )
    .await
}

/// Adds a typed attribute to a source, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn add_source_attribute(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    attribute_type: String,
    value: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute(
        store,
        session,
        &source_id.to_string(),
        SourceCommand::AddAttribute {
            source_id,
            attribute: Attribute {
                attribute_type,
                value,
                citations: Vec::new(),
            },
        },
    )
    .await
}

/// Attaches a media reference (by media aggregate id) to a source, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn attach_source_media(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    media_id: MediaId,
    caption: Option<String>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute(
        store,
        session,
        &source_id.to_string(),
        SourceCommand::AttachMedia {
            source_id,
            media: MediaRef {
                media_id,
                crop: None,
                caption,
                citations: Vec::new(),
            },
        },
    )
    .await
}

/// Attaches a note (by note aggregate id) to a source, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn attach_source_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_id: NoteId,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute(
        store,
        session,
        &source_id.to_string(),
        SourceCommand::AttachNote { source_id, note_id },
    )
    .await
}

/// Applies (or removes) a tag on a source, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn tag_source(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: TagId,
    remove: bool,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    let command = if remove {
        SourceCommand::Untag { source_id, tag_id }
    } else {
        SourceCommand::Tag { source_id, tag_id }
    };
    execute(store, session, &source_id.to_string(), command).await
}

/// Loads a single source's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_source(workspace: &Workspace, human_id: &str) -> Result<Option<SourceSummary>, AppError> {
    let found = workspace.store().find_source(human_id).await?;
    Ok(found.as_ref().map(summarize))
}

/// Lists every source's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_sources(workspace: &Workspace) -> Result<Vec<SourceSummary>, AppError> {
    let views = workspace.store().list_sources().await?;
    let mut summaries = Vec::with_capacity(views.len());
    for view in &views {
        summaries.push(summarize(view));
    }
    Ok(summaries)
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
/// Sets a source's privacy restrictions (GEDCOM `RESN` — data-model §6).
///
/// # Errors
///
/// [`AppError::SourceNotFound`] if no such source exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let source_id = resolve_source_id(store, human_id).await?;
    execute(
        store,
        session,
        &source_id.to_string(),
        SourceCommand::SetRestrictions {
            source_id,
            restrictions,
        },
    )
    .await
}

async fn execute(store: &Store, session: &Session, aggregate_id: &str, command: SourceCommand) -> Result<(), AppError> {
    let envelope = SourceCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_source(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Resolves a `human_id` to its aggregate [`SourceId`], or [`AppError::SourceNotFound`].
async fn resolve_source_id(store: &Store, human_id: &str) -> Result<SourceId, AppError> {
    use_case::resolve_id(store.find_source(human_id).await?, SourceView::source_id, || {
        AppError::SourceNotFound(human_id.to_owned())
    })
}

/// Resolves a repository `human_id` to its aggregate [`RepositoryId`], or
/// [`AppError::RepositoryNotFound`].
async fn resolve_repository_id(store: &Store, human_id: &str) -> Result<RepositoryId, AppError> {
    use_case::resolve_id(
        store.find_repository(human_id).await?,
        RepositoryView::repository_id,
        || AppError::RepositoryNotFound(human_id.to_owned()),
    )
}

/// Renders a [`SourceView`] into the frontend DTO.
fn summarize(view: &SourceView) -> SourceSummary {
    SourceSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        title: view.title().map(ToOwned::to_owned),
        author: view.author().map(ToOwned::to_owned),
        pub_info: view.pub_info().map(ToOwned::to_owned),
        abbrev: view.abbrev().map(ToOwned::to_owned),
        repositories: view
            .repositories()
            .iter()
            .map(|r| r.repository_id.to_string())
            .collect(),
        attributes: view
            .attributes()
            .iter()
            .map(|a| format!("{}={}", a.attribute_type, a.value))
            .collect(),
        restrictions: view.restrictions().clone(),
    }
}
