//! Family use-cases (ADR 0006): create, add/remove partner, add/remove child, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`](genealogy_db::Store),
//! and returns a frontend-neutral [`FamilySummary`] (never a `FamilyView`, cqrs-es, or sqlx type).
//! Partners and children are supplied by Person `human_id` and resolved to a
//! [`PersonId`](genealogy_core::ids::PersonId) here, so the frontend never handles UUIDs. The
//! Family `human_id` is auto-allocated using the workspace's configured format (ADR 0005).

use std::collections::HashMap;

use genealogy_core::enums::ChildParentRelationship;
use genealogy_core::family::command::{FamilyCommand, FamilyCommandEnvelope};
use genealogy_core::family::{FamilyError, FamilyView};
use genealogy_core::ids::{FamilyId, HumanId, PersonId};
use genealogy_core::provenance::Confidence;
use genealogy_db::{CommandError, Store};

use crate::error::AppError;
use crate::session::Session;
use crate::workspace::Workspace;

/// A frontend-neutral summary of a family (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilySummary {
    /// The user-facing identifier (e.g. `F0001`).
    pub human_id: String,
    /// The partners' `human_id`s, resolved from the projected `PersonId`s.
    pub partners: Vec<String>,
    /// The children's `human_id`s, resolved from the projected `PersonId`s.
    pub children: Vec<String>,
    /// Whether the family is marked private.
    pub private: bool,
}

/// Creates a family, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::FamilyDomain`] if a domain rule rejects the command, or a workspace/store error.
pub async fn create_family(workspace: &Workspace, session: &Session) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = store.next_family_human_id(&workspace.family_id_format()?).await?;

    let family_id = session.new_family_id();
    execute(
        store,
        session,
        &family_id.to_string(),
        FamilyCommand::CreateFamily {
            family_id,
            human_id: HumanId::new(&human_id),
        },
    )
    .await?;
    Ok(human_id)
}

/// Adds a partner (by person `human_id`) to the family identified by `family_human_id`.
///
/// # Errors
///
/// [`AppError::FamilyNotFound`]/[`AppError::PersonNotFound`] if either does not exist,
/// [`AppError::FamilyDomain`] if the partner is already present, or a workspace/store error.
pub async fn add_partner(
    workspace: &Workspace,
    session: &Session,
    family_human_id: &str,
    person_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, family_human_id).await?;
    let person_id = resolve_person_id(store, person_human_id).await?;
    execute(
        store,
        session,
        &family_id.to_string(),
        FamilyCommand::AddPartner { family_id, person_id },
    )
    .await
}

/// Removes a partner (by person `human_id`) from the family identified by `family_human_id`.
///
/// # Errors
///
/// As [`add_partner`], but rejects with [`AppError::FamilyDomain`] if the partner is not present.
pub async fn remove_partner(
    workspace: &Workspace,
    session: &Session,
    family_human_id: &str,
    person_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, family_human_id).await?;
    let person_id = resolve_person_id(store, person_human_id).await?;
    execute(
        store,
        session,
        &family_id.to_string(),
        FamilyCommand::RemovePartner { family_id, person_id },
    )
    .await
}

/// Adds a child (by person `human_id`) with a parent relationship to the family.
///
/// # Errors
///
/// As [`add_partner`], but rejects with [`AppError::FamilyDomain`] if the child is already present.
pub async fn add_child(
    workspace: &Workspace,
    session: &Session,
    family_human_id: &str,
    child_human_id: &str,
    relationship: ChildParentRelationship,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, family_human_id).await?;
    let child_id = resolve_person_id(store, child_human_id).await?;
    execute(
        store,
        session,
        &family_id.to_string(),
        FamilyCommand::AddChild {
            family_id,
            child_id,
            relationship,
        },
    )
    .await
}

/// Removes a child (by person `human_id`) from the family.
///
/// # Errors
///
/// As [`add_partner`], but rejects with [`AppError::FamilyDomain`] if the child is not present.
pub async fn remove_child(
    workspace: &Workspace,
    session: &Session,
    family_human_id: &str,
    child_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, family_human_id).await?;
    let child_id = resolve_person_id(store, child_human_id).await?;
    execute(
        store,
        session,
        &family_id.to_string(),
        FamilyCommand::RemoveChild { family_id, child_id },
    )
    .await
}

/// Loads a single family's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_family(workspace: &Workspace, human_id: &str) -> Result<Option<FamilySummary>, AppError> {
    let store = workspace.store();
    let Some(view) = store.find_family(human_id).await? else {
        return Ok(None);
    };
    Ok(Some(summarize(store, &view).await?))
}

/// Lists every family's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_families(workspace: &Workspace) -> Result<Vec<FamilySummary>, AppError> {
    let store = workspace.store();
    let views = store.list_families().await?;
    let mut summaries = Vec::with_capacity(views.len());
    for view in &views {
        summaries.push(summarize(store, view).await?);
    }
    Ok(summaries)
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
async fn execute(store: &Store, session: &Session, aggregate_id: &str, command: FamilyCommand) -> Result<(), AppError> {
    let envelope = FamilyCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_family(aggregate_id, envelope)
        .await
        .map_err(map_command_error)
}

/// Resolves a family `human_id` to its aggregate [`FamilyId`], or [`AppError::FamilyNotFound`].
async fn resolve_family_id(store: &Store, human_id: &str) -> Result<FamilyId, AppError> {
    store
        .find_family(human_id)
        .await?
        .and_then(|view| view.family_id())
        .ok_or_else(|| AppError::FamilyNotFound(human_id.to_owned()))
}

/// Resolves a person `human_id` to its aggregate [`PersonId`], or [`AppError::PersonNotFound`].
async fn resolve_person_id(store: &Store, human_id: &str) -> Result<PersonId, AppError> {
    store
        .find_person(human_id)
        .await?
        .and_then(|view| view.person_id())
        .ok_or_else(|| AppError::PersonNotFound(human_id.to_owned()))
}

/// Renders a [`FamilyView`] into the frontend DTO, resolving member `PersonId`s back to `human_id`s.
///
/// A member whose person projection is missing (a dangling reference) renders as its UUID rather
/// than failing the whole read.
async fn summarize(store: &Store, view: &FamilyView) -> Result<FamilySummary, AppError> {
    let persons: HashMap<PersonId, String> = store
        .list_persons()
        .await?
        .iter()
        .filter_map(|p| Some((p.person_id()?, p.human_id()?.to_string())))
        .collect();
    let resolve = |person_id: PersonId| {
        persons
            .get(&person_id)
            .cloned()
            .unwrap_or_else(|| person_id.to_string())
    };

    let partners = view.partners().into_iter().map(resolve).collect();
    let children = view.children().iter().map(|c| resolve(c.child_id)).collect();
    let human_id = view.human_id().map(ToString::to_string).unwrap_or_default();
    Ok(FamilySummary {
        human_id,
        partners,
        children,
        private: view.is_private(),
    })
}

/// Maps a [`CommandError`] to [`AppError`], keeping a domain rejection distinct from infrastructure.
fn map_command_error(error: CommandError<FamilyError>) -> AppError {
    match error {
        CommandError::Rejected(domain) => AppError::FamilyDomain(domain),
        CommandError::Store(db) => AppError::Db(db),
    }
}
