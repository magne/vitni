//! Citation use-cases (ADR 0006): create (against a source), set page, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`], and returns a
//! frontend-neutral [`CitationSummary`]. Creating a citation resolves the cited source's `human_id`
//! to its id (an [`AppError::SourceNotFound`] if absent); the core then *also* re-checks the source
//! exists against the projection via the aggregate's `Services` resolver, surfacing
//! [`CitationError::UnknownSource`](genealogy_core::citation::CitationError) — the §9 aggregate-tax
//! check (ADR 0004 §3).

use std::collections::HashMap;

use genealogy_core::citation::command::{CitationCommand, CitationCommandEnvelope};
use genealogy_core::citation::{CitationError, CitationView};
use genealogy_core::ids::{CitationId, HumanId, SourceId};
use genealogy_core::provenance::Confidence;
use genealogy_db::{CommandError, Store};

use crate::error::AppError;
use crate::session::Session;
use crate::workspace::Workspace;

/// A frontend-neutral summary of a citation (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationSummary {
    /// The user-facing identifier (e.g. `C0001`).
    pub human_id: String,
    /// The cited source's `human_id`, resolved from the projected `SourceId`.
    pub source: Option<String>,
    /// The page / locator within the source, if set.
    pub page: Option<String>,
}

/// What to create a citation with (the auto/override `human_id`, the cited source, and a page).
#[derive(Debug, Clone)]
pub struct NewCitation {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// The cited source's `human_id` (e.g. `S0001`).
    pub source: String,
    /// An optional page / locator for an initial `SetPage`.
    pub page: Option<String>,
}

/// Creates a citation against a source, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::SourceNotFound`] if the cited
/// source does not exist, [`AppError::CitationDomain`] if a domain rule rejects the command (e.g.
/// `UnknownSource`), or a workspace/store error.
pub async fn create_citation(workspace: &Workspace, session: &Session, new: NewCitation) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_citation(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_citation_human_id(&workspace.citation_id_format()?).await?,
    };

    let source_id = resolve_source_id(store, &new.source).await?;
    let citation_id = session.new_citation_id();
    let aggregate_id = citation_id.to_string();

    execute(
        store,
        session,
        &aggregate_id,
        CitationCommand::CreateCitation {
            citation_id,
            human_id: HumanId::new(&human_id),
            source_id,
        },
    )
    .await?;

    if let Some(page) = new.page {
        execute(
            store,
            session,
            &aggregate_id,
            CitationCommand::SetPage { citation_id, page },
        )
        .await?;
    }

    Ok(human_id)
}

/// Sets (or changes) an existing citation's page, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::CitationNotFound`] if no such citation exists, or a workspace/store error.
pub async fn set_page(workspace: &Workspace, session: &Session, human_id: &str, page: String) -> Result<(), AppError> {
    let store = workspace.store();
    let citation_id = resolve_citation_id(store, human_id).await?;
    execute(
        store,
        session,
        &citation_id.to_string(),
        CitationCommand::SetPage { citation_id, page },
    )
    .await
}

/// Loads a single citation's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_citation(workspace: &Workspace, human_id: &str) -> Result<Option<CitationSummary>, AppError> {
    let store = workspace.store();
    let Some(view) = store.find_citation(human_id).await? else {
        return Ok(None);
    };
    let sources = source_human_ids(store).await?;
    Ok(Some(summarize(&view, &sources)))
}

/// Lists every citation's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_citations(workspace: &Workspace) -> Result<Vec<CitationSummary>, AppError> {
    let store = workspace.store();
    let views = store.list_citations().await?;
    let sources = source_human_ids(store).await?;
    Ok(views.iter().map(|view| summarize(view, &sources)).collect())
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: CitationCommand,
) -> Result<(), AppError> {
    let envelope = CitationCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_citation(aggregate_id, envelope)
        .await
        .map_err(map_command_error)
}

/// Resolves a source `human_id` to its aggregate [`SourceId`], or [`AppError::SourceNotFound`].
async fn resolve_source_id(store: &Store, human_id: &str) -> Result<SourceId, AppError> {
    let view = store
        .find_source(human_id)
        .await?
        .ok_or_else(|| AppError::SourceNotFound(human_id.to_owned()))?;
    view.source_id()
        .ok_or_else(|| AppError::SourceNotFound(human_id.to_owned()))
}

/// Resolves a citation `human_id` to its aggregate [`CitationId`], or [`AppError::CitationNotFound`].
async fn resolve_citation_id(store: &Store, human_id: &str) -> Result<CitationId, AppError> {
    let view = store
        .find_citation(human_id)
        .await?
        .ok_or_else(|| AppError::CitationNotFound(human_id.to_owned()))?;
    view.citation_id()
        .ok_or_else(|| AppError::CitationNotFound(human_id.to_owned()))
}

/// Builds a `SourceId -> human_id` lookup from the Source projection, to render the cited source.
async fn source_human_ids(store: &Store) -> Result<HashMap<SourceId, String>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_sources().await? {
        if let (Some(id), Some(human_id)) = (view.source_id(), view.human_id()) {
            map.insert(id, human_id.as_str().to_owned());
        }
    }
    Ok(map)
}

/// Renders a [`CitationView`] into the frontend DTO, resolving the cited source's `human_id`.
fn summarize(view: &CitationView, sources: &HashMap<SourceId, String>) -> CitationSummary {
    let source = view.source_id().and_then(|id| sources.get(&id).cloned());
    CitationSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        source,
        page: view.page().map(ToOwned::to_owned),
    }
}

/// Maps a [`CommandError`] to [`AppError`], keeping a domain rejection distinct from infrastructure.
fn map_command_error(error: CommandError<CitationError>) -> AppError {
    match error {
        CommandError::Rejected(domain) => AppError::CitationDomain(domain),
        CommandError::Store(db) => AppError::Db(db),
    }
}
