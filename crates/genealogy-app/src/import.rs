//! Import use-cases (ADR 0006, ADR 0013): resolve-or-create against an [`ExternalId`], applied
//! additively so re-importing an unchanged record produces no events.
//!
//! A bulk importer (the plugin host's `commands` capability) does not mint aggregates directly; it
//! calls these use-cases with the `(authority, value)` it parsed from each record. An incoming
//! record is resolved to its existing aggregate by that key (data-model §11) instead of creating a
//! duplicate. Updates are **additive**: a new person/family/link is created, an identical one is a
//! no-op, and a conflicting single-valued fact is left untouched (true merge is deferred).

use genealogy_core::enums::{ChildParentRelationship, EvidenceLevel};
use genealogy_core::family::FamilyError;
use genealogy_core::person::PersonView;
use genealogy_core::text::ExternalId;
use genealogy_db::DbError;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case::Provenance;
use crate::workspace::Workspace;
use crate::{family, person};

/// Resolves a person by `external_id`, or creates one — returning its `human_id` and whether it was
/// newly created.
///
/// An existing person (resolved by the identifier) keeps its identity; the incoming name is added
/// only if it is not already asserted (additive — a divergent name is left as a separate name, never
/// an overwrite). A new person is created with the name and tagged with the identifier.
///
/// # Errors
///
/// [`AppError::Domain`] if a name is empty, or a workspace/store error.
pub async fn import_person(
    workspace: &Workspace,
    session: &Session,
    external_id: ExternalId,
    name: Option<person::PersonNameParts>,
) -> Result<(String, bool), AppError> {
    let store = workspace.store();
    if let Some(view) = store
        .find_person_by_external_id(&external_id.authority, &external_id.value)
        .await?
    {
        let human_id = human_id_of(view.human_id(), "person")?;
        ensure_name(workspace, session, &view, &human_id, name).await?;
        return Ok((human_id, false));
    }

    let human_id = person::create_person(
        workspace,
        session,
        person::NewPerson {
            human_id: None,
            name,
            evidence_level: EvidenceLevel::Persona,
        },
    )
    .await?;
    person::add_external_id(workspace, session, &human_id, external_id).await?;
    Ok((human_id, true))
}

/// Resolves a family by `external_id`, or creates one — returning its `human_id` and whether it was
/// newly created.
///
/// # Errors
///
/// A workspace/store error.
pub async fn import_family(
    workspace: &Workspace,
    session: &Session,
    external_id: ExternalId,
) -> Result<(String, bool), AppError> {
    let store = workspace.store();
    if let Some(view) = store
        .find_family_by_external_id(&external_id.authority, &external_id.value)
        .await?
    {
        return Ok((human_id_of(view.human_id(), "family")?, false));
    }

    let human_id = family::create_family(workspace, session).await?;
    family::add_external_id(workspace, session, &human_id, external_id).await?;
    Ok((human_id, true))
}

/// Adds a partner to a family, treating an already-present partner as a no-op (additive re-import).
///
/// # Errors
///
/// [`AppError::FamilyNotFound`]/[`AppError::PersonNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_add_partner(
    workspace: &Workspace,
    session: &Session,
    family_human_id: &str,
    person_human_id: &str,
) -> Result<(), AppError> {
    match family::add_partner(workspace, session, family_human_id, person_human_id).await {
        Err(AppError::FamilyDomain(FamilyError::PartnerAlreadyPresent(_))) => Ok(()),
        other => other,
    }
}

/// Adds a (birth) child to a family, treating an already-present child as a no-op (additive
/// re-import).
///
/// # Errors
///
/// [`AppError::FamilyNotFound`]/[`AppError::PersonNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_add_child(
    workspace: &Workspace,
    session: &Session,
    family_human_id: &str,
    child_human_id: &str,
) -> Result<(), AppError> {
    match family::add_child(
        workspace,
        session,
        family_human_id,
        child_human_id,
        ChildParentRelationship::Birth,
    )
    .await
    {
        Err(AppError::FamilyDomain(FamilyError::ChildAlreadyPresent(_))) => Ok(()),
        other => other,
    }
}

/// Asserts the incoming name on an existing person only if that exact name is not already present.
///
/// This is what keeps a re-import additive: an identical name yields no event; a genuinely new name
/// is added alongside the existing ones.
async fn ensure_name(
    workspace: &Workspace,
    session: &Session,
    view: &PersonView,
    human_id: &str,
    name: Option<person::PersonNameParts>,
) -> Result<(), AppError> {
    let Some(name) = name.filter(|parts| !parts.is_empty()) else {
        return Ok(());
    };
    let candidate = person::build_name(name.clone());
    if view.names().into_iter().any(|existing| *existing == candidate) {
        return Ok(());
    }
    person::add_name(workspace, session, human_id, name, Provenance::default(), &[]).await
}

/// Pulls the `human_id` string from a just-resolved view, mapping a missing one to a backend error
/// (the projection is internally inconsistent if a created aggregate has no `human_id`).
fn human_id_of(human_id: Option<&genealogy_core::ids::HumanId>, noun: &str) -> Result<String, AppError> {
    human_id
        .map(|h| h.as_str().to_owned())
        .ok_or_else(|| AppError::Db(DbError::Backend(format!("resolved {noun} projection has no human_id"))))
}
