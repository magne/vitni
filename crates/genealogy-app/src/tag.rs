//! Tag use-cases (ADR 0006): create, rename, set colour/priority, show, and list.
//!
//! Tags carry no `HumanId` (data-model §9); they are identified by their aggregate id (a UUID
//! string). `create_tag` returns the new id so the caller can reference it when tagging.

use std::collections::BTreeSet;

use genealogy_core::enums::Restriction;
use genealogy_core::ids::TagId;
use genealogy_core::provenance::CitationRef;
use genealogy_core::tag::TagView;
use genealogy_core::tag::command::{TagCommand, TagCommandEnvelope};
use genealogy_db::Store;
use uuid::Uuid;

use crate::error::AppError;
use crate::session::Session;
use crate::tag_usage::{TagUsage, TagUsageGroup};
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// A frontend-neutral summary of a tag (the DTO the CLI renders), carrying its stable id and — for a
/// single-tag view — the joined Usage breakdown (the cross-aggregate-joins dependency note).
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
    /// How many records carry this tag in total (the list-row `· N objects` subtitle and the detail
    /// header count). Filled by both [`list_tags`] and [`show_tag`] from a single reverse-index scan.
    pub usage_count: usize,
    /// The records carrying this tag, grouped by object type (the Usage tab). Populated by
    /// [`show_tag`]; empty in [`list_tags`] (only the aggregate [`usage_count`](Self::usage_count) is
    /// filled there — the per-group examples would need the full join rendered).
    pub usage: Vec<TagUsageGroup>,
    /// The tag's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// Creates a tag with `name`, returning its new aggregate id (a UUID string).
///
/// # Errors
///
/// [`AppError::TagDomain`] if the name is empty, or a workspace/store error.
pub async fn create_tag(
    workspace: &Workspace,
    session: &Session,
    name: String,
    provenance: Provenance,
    citations: &[String],
) -> Result<String, AppError> {
    let store = workspace.store();
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;
    let tag_id = session.new_tag_id();
    let aggregate_id = tag_id.to_string();
    execute(
        store,
        session,
        &aggregate_id,
        TagCommand::CreateTag { tag_id, name },
        provenance,
        citation_refs,
    )
    .await?;
    Ok(aggregate_id)
}

/// Renames a tag, identified by its aggregate id.
///
/// # Errors
///
/// [`AppError::TagNotFound`] if the id is unknown or malformed, [`AppError::TagDomain`] if the name
/// is empty, or a workspace/store error.
pub async fn rename_tag(
    workspace: &Workspace,
    session: &Session,
    id: &str,
    name: String,
    provenance: Provenance,
    citations: &[String],
) -> Result<(), AppError> {
    let store = workspace.store();
    let tag_id = parse_tag_id(id)?;
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;
    execute(
        store,
        session,
        id,
        TagCommand::RenameTag { tag_id, name },
        provenance,
        citation_refs,
    )
    .await
}

/// Sets (or changes) a tag's colour, identified by its aggregate id.
///
/// # Errors
///
/// [`AppError::TagNotFound`] if the id is unknown or malformed, or a workspace/store error.
pub async fn set_tag_color(
    workspace: &Workspace,
    session: &Session,
    id: &str,
    color: String,
    provenance: Provenance,
    citations: &[String],
) -> Result<(), AppError> {
    let store = workspace.store();
    let tag_id = parse_tag_id(id)?;
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;
    execute(
        store,
        session,
        id,
        TagCommand::SetTagColor { tag_id, color },
        provenance,
        citation_refs,
    )
    .await
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
    provenance: Provenance,
    citations: &[String],
) -> Result<(), AppError> {
    let store = workspace.store();
    let tag_id = parse_tag_id(id)?;
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;
    execute(
        store,
        session,
        id,
        TagCommand::SetTagPriority { tag_id, priority },
        provenance,
        citation_refs,
    )
    .await
}

/// Sets a tag's privacy restrictions (GEDCOM `RESN` — data-model §6), identified by its aggregate id.
///
/// # Errors
///
/// [`AppError::TagNotFound`] if the id is unknown or malformed, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    id: &str,
    restrictions: BTreeSet<Restriction>,
    provenance: Provenance,
    citations: &[String],
) -> Result<(), AppError> {
    let store = workspace.store();
    let tag_id = parse_tag_id(id)?;
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;
    execute(
        store,
        session,
        id,
        TagCommand::SetRestrictions { tag_id, restrictions },
        provenance,
        citation_refs,
    )
    .await
}

/// Loads a single tag's summary by its aggregate id.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_tag(workspace: &Workspace, id: &str) -> Result<Option<TagSummary>, AppError> {
    let Some(view) = workspace.store().find_tag(id).await? else {
        return Ok(None);
    };
    let (usage, usage_count) = match view.tag_id() {
        Some(tag_id) => {
            let index = TagUsage::load(workspace).await?;
            (index.groups(tag_id), index.count(tag_id))
        }
        None => (Vec::new(), 0),
    };
    Ok(Some(summarize(&view, usage, usage_count)))
}

/// Lists every tag's summary (without the per-tag Usage breakdown — see [`show_tag`]).
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_tags(workspace: &Workspace) -> Result<Vec<TagSummary>, AppError> {
    let views = workspace.store().list_tags().await?;
    let index = TagUsage::load(workspace).await?;
    let mut summaries = Vec::with_capacity(views.len());
    for view in &views {
        let usage_count = view.tag_id().map_or(0, |tag_id| index.count(tag_id));
        summaries.push(summarize(view, Vec::new(), usage_count));
    }
    Ok(summaries)
}

/// Executes one command through the store, stamping the operator `provenance` and backing
/// `citations`, and mapping the command outcome to [`AppError`]. Tags carry no supersede path
/// (data-model §9), so a correction is not expressed here.
async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: TagCommand,
    provenance: Provenance,
    citations: Vec<CitationRef>,
) -> Result<(), AppError> {
    let envelope = TagCommandEnvelope {
        meta: session.new_meta(provenance, citations),
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

/// Renders a [`TagView`] into the frontend DTO, with the pre-computed Usage breakdown.
fn summarize(view: &TagView, usage: Vec<TagUsageGroup>, usage_count: usize) -> TagSummary {
    TagSummary {
        id: view.tag_id().map(|id| id.to_string()).unwrap_or_default(),
        name: view.name().map(ToOwned::to_owned),
        color: view.color().map(ToOwned::to_owned),
        priority: view.priority(),
        usage_count,
        usage,
        restrictions: view.restrictions().clone(),
    }
}
