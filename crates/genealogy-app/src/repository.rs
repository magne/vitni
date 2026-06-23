//! Repository use-cases (ADR 0006): create, set type/name, add address/url, attach note, tag,
//! show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`RepositorySummary`]. `human_id` is auto-allocated using the workspace's
//! configured format, or validated when supplied (ADR 0005).

use std::collections::BTreeSet;

use genealogy_core::address::Address;
use genealogy_core::enums::{RepositoryType, Restriction};
use genealogy_core::ids::{HumanId, NoteId, RepositoryId, TagId};
use genealogy_core::provenance::Confidence;
use genealogy_core::repository::RepositoryView;
use genealogy_core::repository::command::{RepositoryCommand, RepositoryCommandEnvelope};
use genealogy_core::text::Url;
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case;
use crate::workspace::Workspace;

/// A frontend-neutral summary of a repository (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositorySummary {
    /// The user-facing identifier (e.g. `R0001`).
    pub human_id: String,
    /// The repository's type. Structured (not a label) so the frontend localizes it (ADR 0003).
    pub repository_type: Option<RepositoryType>,
    /// The repository's name, if set.
    pub name: Option<String>,
    /// The number of recorded addresses.
    pub address_count: usize,
    /// The number of recorded URLs.
    pub url_count: usize,
    /// The repository's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// What to create a repository with (the auto/override `human_id` and an optional name).
#[derive(Debug, Clone)]
pub struct NewRepository {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// An optional name for an initial `SetName`.
    pub name: Option<String>,
}

/// Creates a repository, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::RepositoryDomain`] if a
/// domain rule rejects the command, or a workspace/store error.
pub async fn create_repository(
    workspace: &Workspace,
    session: &Session,
    new: NewRepository,
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_repository(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => {
            store
                .next_repository_human_id(&workspace.repository_id_format()?)
                .await?
        }
    };

    let repository_id = session.new_repository_id();
    let aggregate_id = repository_id.to_string();

    execute(
        store,
        session,
        &aggregate_id,
        RepositoryCommand::CreateRepository {
            repository_id,
            human_id: HumanId::new(&human_id),
        },
    )
    .await?;

    if let Some(name) = new.name {
        execute(
            store,
            session,
            &aggregate_id,
            RepositoryCommand::SetName { repository_id, name },
        )
        .await?;
    }

    Ok(human_id)
}

/// Sets (or changes) a repository's type, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or a workspace/store error.
pub async fn set_repository_type(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    repository_type: RepositoryType,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    execute(
        store,
        session,
        &repository_id.to_string(),
        RepositoryCommand::SetRepositoryType {
            repository_id,
            repository_type,
        },
    )
    .await
}

/// Sets (or changes) a repository's name, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, [`AppError::RepositoryDomain`] if
/// the name is empty, or a workspace/store error.
pub async fn set_repository_name(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    name: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    execute(
        store,
        session,
        &repository_id.to_string(),
        RepositoryCommand::SetName { repository_id, name },
    )
    .await
}

/// Adds a postal address to a repository, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or a workspace/store error.
pub async fn add_repository_address(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    address: Address,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    execute(
        store,
        session,
        &repository_id.to_string(),
        RepositoryCommand::AddAddress { repository_id, address },
    )
    .await
}

/// Adds a URL to a repository, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or a workspace/store error.
pub async fn add_repository_url(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    url: Url,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    execute(
        store,
        session,
        &repository_id.to_string(),
        RepositoryCommand::AddUrl { repository_id, url },
    )
    .await
}

/// Attaches a note (by note aggregate id) to a repository, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or a workspace/store error.
pub async fn attach_repository_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_id: NoteId,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    execute(
        store,
        session,
        &repository_id.to_string(),
        RepositoryCommand::AttachNote { repository_id, note_id },
    )
    .await
}

/// Applies (or removes) a tag on a repository, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or a workspace/store error.
pub async fn tag_repository(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: TagId,
    remove: bool,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    let command = if remove {
        RepositoryCommand::Untag { repository_id, tag_id }
    } else {
        RepositoryCommand::Tag { repository_id, tag_id }
    };
    execute(store, session, &repository_id.to_string(), command).await
}

/// Loads a single repository's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_repository(workspace: &Workspace, human_id: &str) -> Result<Option<RepositorySummary>, AppError> {
    let found = workspace.store().find_repository(human_id).await?;
    Ok(found.as_ref().map(summarize))
}

/// Lists every repository's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_repositories(workspace: &Workspace) -> Result<Vec<RepositorySummary>, AppError> {
    let views = workspace.store().list_repositories().await?;
    let mut summaries = Vec::with_capacity(views.len());
    for view in &views {
        summaries.push(summarize(view));
    }
    Ok(summaries)
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
/// Sets a repository's privacy restrictions (GEDCOM `RESN` — data-model §6).
///
/// # Errors
///
/// [`AppError::RepositoryNotFound`] if no such repository exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let repository_id = resolve_repository_id(store, human_id).await?;
    execute(
        store,
        session,
        &repository_id.to_string(),
        RepositoryCommand::SetRestrictions {
            repository_id,
            restrictions,
        },
    )
    .await
}

async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: RepositoryCommand,
) -> Result<(), AppError> {
    let envelope = RepositoryCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_repository(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Resolves a `human_id` to its aggregate [`RepositoryId`], or [`AppError::RepositoryNotFound`].
async fn resolve_repository_id(store: &Store, human_id: &str) -> Result<RepositoryId, AppError> {
    use_case::resolve_id(
        store.find_repository(human_id).await?,
        RepositoryView::repository_id,
        || AppError::RepositoryNotFound(human_id.to_owned()),
    )
}

/// Renders a [`RepositoryView`] into the frontend DTO.
fn summarize(view: &RepositoryView) -> RepositorySummary {
    RepositorySummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        repository_type: view.repository_type().cloned(),
        name: view.name().map(ToOwned::to_owned),
        address_count: view.addresses().len(),
        url_count: view.urls().len(),
        restrictions: view.restrictions().clone(),
    }
}
