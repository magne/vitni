//! Source use-cases (ADR 0006): create, set title, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`SourceSummary`]. `human_id` is auto-allocated using the workspace's configured
//! format, or validated when supplied (ADR 0005).

use genealogy_core::ids::{HumanId, SourceId};
use genealogy_core::provenance::Confidence;
use genealogy_core::source::command::{SourceCommand, SourceCommandEnvelope};
use genealogy_core::source::{SourceError, SourceView};
use genealogy_db::{CommandError, Store};

use crate::error::AppError;
use crate::session::Session;
use crate::workspace::Workspace;

/// A frontend-neutral summary of a source (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceSummary {
    /// The user-facing identifier (e.g. `S0001`).
    pub human_id: String,
    /// The bibliographic title, if set.
    pub title: Option<String>,
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
async fn execute(store: &Store, session: &Session, aggregate_id: &str, command: SourceCommand) -> Result<(), AppError> {
    let envelope = SourceCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_source(aggregate_id, envelope)
        .await
        .map_err(map_command_error)
}

/// Resolves a `human_id` to its aggregate [`SourceId`], or [`AppError::SourceNotFound`].
async fn resolve_source_id(store: &Store, human_id: &str) -> Result<SourceId, AppError> {
    let view = store
        .find_source(human_id)
        .await?
        .ok_or_else(|| AppError::SourceNotFound(human_id.to_owned()))?;
    view.source_id()
        .ok_or_else(|| AppError::SourceNotFound(human_id.to_owned()))
}

/// Renders a [`SourceView`] into the frontend DTO.
fn summarize(view: &SourceView) -> SourceSummary {
    SourceSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        title: view.title().map(ToOwned::to_owned),
    }
}

/// Maps a [`CommandError`] to [`AppError`], keeping a domain rejection distinct from infrastructure.
fn map_command_error(error: CommandError<SourceError>) -> AppError {
    match error {
        CommandError::Rejected(domain) => AppError::SourceDomain(domain),
        CommandError::Store(db) => AppError::Db(db),
    }
}
