//! Tag use-cases (ADR 0006): create, rename, set colour/priority, show, and list.
//!
//! Tags carry no `HumanId` (data-model §9); they are identified by their aggregate id (a UUID
//! string). `create_tag` returns the new id so the caller can reference it when tagging.

use genealogy_core::ids::TagId;
use genealogy_core::provenance::Confidence;
use genealogy_core::tag::TagView;
use genealogy_core::tag::command::{TagCommand, TagCommandEnvelope};
use genealogy_db::Store;
use uuid::Uuid;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case;
use crate::workspace::Workspace;

/// A frontend-neutral summary of a tag (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagSummary {
    /// The tag's aggregate id (a UUID string; tags have no `human_id`).
    pub id: String,
    /// The tag's name, if set.
    pub name: Option<String>,
    /// The tag's colour, if set.
    pub color: Option<String>,
    /// The tag's sort priority, if set.
    pub priority: Option<i32>,
}

/// Creates a tag with `name`, returning its new aggregate id (a UUID string).
///
/// # Errors
///
/// [`AppError::TagDomain`] if the name is empty, or a workspace/store error.
pub async fn create_tag(workspace: &Workspace, session: &Session, name: String) -> Result<String, AppError> {
    let store = workspace.store();
    let tag_id = session.new_tag_id();
    let aggregate_id = tag_id.to_string();
    execute(store, session, &aggregate_id, TagCommand::CreateTag { tag_id, name }).await?;
    Ok(aggregate_id)
}

/// Renames a tag, identified by its aggregate id.
///
/// # Errors
///
/// [`AppError::TagNotFound`] if the id is unknown or malformed, [`AppError::TagDomain`] if the name
/// is empty, or a workspace/store error.
pub async fn rename_tag(workspace: &Workspace, session: &Session, id: &str, name: String) -> Result<(), AppError> {
    let store = workspace.store();
    let tag_id = parse_tag_id(id)?;
    execute(store, session, id, TagCommand::RenameTag { tag_id, name }).await
}

/// Sets (or changes) a tag's colour, identified by its aggregate id.
///
/// # Errors
///
/// [`AppError::TagNotFound`] if the id is unknown or malformed, or a workspace/store error.
pub async fn set_tag_color(workspace: &Workspace, session: &Session, id: &str, color: String) -> Result<(), AppError> {
    let store = workspace.store();
    let tag_id = parse_tag_id(id)?;
    execute(store, session, id, TagCommand::SetTagColor { tag_id, color }).await
}

/// Sets (or changes) a tag's sort priority, identified by its aggregate id.
///
/// # Errors
///
/// [`AppError::TagNotFound`] if the id is unknown or malformed, or a workspace/store error.
pub async fn set_tag_priority(
    workspace: &Workspace,
    session: &Session,
    id: &str,
    priority: i32,
) -> Result<(), AppError> {
    let store = workspace.store();
    let tag_id = parse_tag_id(id)?;
    execute(store, session, id, TagCommand::SetTagPriority { tag_id, priority }).await
}

/// Loads a single tag's summary by its aggregate id.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_tag(workspace: &Workspace, id: &str) -> Result<Option<TagSummary>, AppError> {
    let found = workspace.store().find_tag(id).await?;
    Ok(found.as_ref().map(summarize))
}

/// Lists every tag's summary.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_tags(workspace: &Workspace) -> Result<Vec<TagSummary>, AppError> {
    let views = workspace.store().list_tags().await?;
    let mut summaries = Vec::with_capacity(views.len());
    for view in &views {
        summaries.push(summarize(view));
    }
    Ok(summaries)
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
async fn execute(store: &Store, session: &Session, aggregate_id: &str, command: TagCommand) -> Result<(), AppError> {
    let envelope = TagCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_tag(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Parses a tag aggregate id (a UUID string), or [`AppError::TagNotFound`] if malformed.
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// Renders a [`TagView`] into the frontend DTO.
fn summarize(view: &TagView) -> TagSummary {
    TagSummary {
        id: view.tag_id().map(|id| id.to_string()).unwrap_or_default(),
        name: view.name().map(ToOwned::to_owned),
        color: view.color().map(ToOwned::to_owned),
        priority: view.priority(),
    }
}
