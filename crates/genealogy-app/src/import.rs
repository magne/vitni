//! Import use-cases (ADR 0006, ADR 0013): resolve-or-create against an [`ExternalId`], applied
//! additively so re-importing an unchanged record produces no events.
//!
//! A bulk importer (the plugin host's `commands` capability) does not mint aggregates directly; it
//! calls these use-cases with the `(authority, value)` it parsed from each record. An incoming
//! record is resolved to its existing aggregate by that key (data-model §11) instead of creating a
//! duplicate. Updates are **additive** for most fields: a new person/family/link is created, an
//! identical one is a no-op, and a conflicting single-valued fact is left untouched. [`import_assert_sex`]
//! is the one exception (ADR 0029, the first — and so far only — field this timestamp-gated
//! reconciliation rule covers): a differing live `Person.sex` is superseded, not just left alone,
//! when the file's own export date is at least as current as the live assertion.

use genealogy_core::enums::{EvidenceLevel, Sex};
use genealogy_core::family::FamilyError;
use genealogy_core::person::PersonView;
use genealogy_core::provenance::Timestamp;
use genealogy_core::text::ExternalId;
use genealogy_db::DbError;

use crate::error::AppError;
use crate::history::assertion_occurred_at;
use crate::session::Session;
use crate::use_case::{MutationMeta, Provenance};
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
        Provenance::default(),
        &[],
    )
    .await?;
    person::add_external_id(workspace, session, &human_id, external_id, MutationMeta::default()).await?;
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

    let human_id = family::create_family(workspace, session, Provenance::default(), &[]).await?;
    family::add_external_id(workspace, session, &human_id, external_id, MutationMeta::default()).await?;
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
    match family::add_partner(
        workspace,
        session,
        family_human_id,
        person_human_id,
        MutationMeta::default(),
    )
    .await
    {
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
    relationships: Vec<(String, genealogy_core::enums::ChildParentRelationship)>,
) -> Result<(), AppError> {
    // Membership and each parent link are separate assertions (ADR 0021), so re-import swallows a
    // duplicate *per piece*: an already-present child keeps its membership, and a newly-appearing
    // `_FREL`/`_MREL` link is still added. Adding the child with no relationships asserts membership
    // only; a plain GEDCOM `CHIL` / Gramps `<childref>` with no pedigree supplies none.
    match family::add_child(
        workspace,
        session,
        family_human_id,
        child_human_id,
        Vec::new(),
        MutationMeta::default(),
    )
    .await
    {
        Err(AppError::FamilyDomain(FamilyError::ChildAlreadyPresent(_))) => {}
        other => other?,
    }
    for (partner_human_id, relationship) in relationships {
        match family::assert_child_relationship(
            workspace,
            session,
            family_human_id,
            child_human_id,
            &partner_human_id,
            relationship,
            MutationMeta::default(),
        )
        .await
        {
            Err(AppError::FamilyDomain(FamilyError::ChildRelationshipAlreadyPresent(..))) => {}
            other => other?,
        }
    }
    Ok(())
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
    person::add_name(workspace, session, human_id, name, MutationMeta::default()).await
}

/// Asserts a person's sex during import, reconciling against any existing value using the file's
/// own export date (ADR 0029 — the one field this PR's timestamp-gated rule covers): a person with
/// no live sex assertion yet gets it asserted plainly (additive — new information landing on an
/// existing record); an already-matching live value is a no-op (idempotent re-import); a differing
/// live value is superseded only when `file_asserted_at` is known and the live assertion's
/// `occurred_at` is at or before it (the file is at least as current as what is stored) — otherwise,
/// including when `file_asserted_at` is `None` (a missing or unparseable export date), the
/// workspace's value is left untouched (today's additive-only behavior, ADR 0029 §3: honest about
/// carrying no structure rather than guessing).
///
/// `provenance` is the caller's confidence template (ADR 0017 §7 — `None` for a plain bulk import,
/// `Some(Confidence::Low)` for an assisted-import session), threaded through unchanged to whichever
/// path below actually asserts.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, or a workspace/store error.
pub async fn import_assert_sex(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    sex: Sex,
    file_asserted_at: Option<Timestamp>,
    provenance: Provenance,
) -> Result<(), AppError> {
    let store = workspace.store();
    let view = store
        .find_person(human_id)
        .await?
        .ok_or_else(|| AppError::PersonNotFound(human_id.to_owned()))?;
    let Some(live) = view.sex_with_assertions().last() else {
        let meta = MutationMeta {
            provenance,
            ..MutationMeta::default()
        };
        return person::assert_sex(workspace, session, human_id, sex, meta).await;
    };
    if live.value.value == sex {
        return Ok(());
    }
    let Some(file_asserted_at) = file_asserted_at else {
        return Ok(());
    };
    let person_id = view
        .person_id()
        .ok_or_else(|| AppError::PersonNotFound(human_id.to_owned()))?;
    let occurred_at = assertion_occurred_at(store, "person", &person_id.to_string(), live.assertion_id).await?;
    if occurred_at.is_none_or(|occurred_at| occurred_at > file_asserted_at) {
        return Ok(());
    }
    let target = live.assertion_id.to_string();
    let meta = MutationMeta {
        provenance,
        supersedes: Some(&target),
        ..MutationMeta::default()
    };
    person::assert_sex(workspace, session, human_id, sex, meta).await
}

/// Pulls the `human_id` string from a just-resolved view, mapping a missing one to a backend error
/// (the projection is internally inconsistent if a created aggregate has no `human_id`).
fn human_id_of(human_id: Option<&genealogy_core::ids::HumanId>, noun: &str) -> Result<String, AppError> {
    human_id
        .map(|h| h.as_str().to_owned())
        .ok_or_else(|| AppError::Db(DbError::Backend(format!("resolved {noun} projection has no human_id"))))
}
