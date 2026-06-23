//! Family use-cases (ADR 0006): create, add/remove partner, add/remove child, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`](genealogy_db::Store),
//! and returns a frontend-neutral [`FamilySummary`] (never a `FamilyView`, cqrs-es, or sqlx type).
//! Partners and children are supplied by Person `human_id` and resolved to a
//! [`PersonId`](genealogy_core::ids::PersonId) here, so the frontend never handles UUIDs. The
//! Family `human_id` is auto-allocated using the workspace's configured format (ADR 0005).

use std::collections::{BTreeSet, HashMap};

use genealogy_core::enums::{ChildParentRelationship, Restriction};
use genealogy_core::family::FamilyView;
use genealogy_core::family::command::{FamilyCommand, FamilyCommandEnvelope};
use genealogy_core::ids::{CitationId, FamilyId, HumanId, MediaId, NoteId, PersonId, TagId};
use genealogy_core::person::PersonView;
use genealogy_core::provenance::Confidence;
use genealogy_core::text::{ExternalId, MediaRef};
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case;
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
    /// `human_id`s of citations backing the family's claims (e.g. `FAM.SOUR`), in assertion order.
    pub citations: Vec<String>,
    /// `human_id`s of media attached to the family (e.g. `FAM.OBJE`), in assertion order.
    pub media: Vec<String>,
    /// `human_id`s of notes attached to the family (e.g. `FAM.NOTE`), in assertion order.
    pub notes: Vec<String>,
    /// Ids of tags applied to the family, in assertion order.
    pub tags: Vec<String>,
    /// The family's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// A person's role within a family: a partner/spouse, or a child (with the parent relationship).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonFamilyRole {
    /// The person is a partner/spouse in the family.
    Partner,
    /// The person is a child in the family, with the recorded parent relationship.
    Child(ChildParentRelationship),
}

/// A family a person belongs to, annotated with the person's role in it (the Person "Families" view).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyForPerson {
    /// The family's `human_id` (e.g. `F0001`).
    pub family_human_id: String,
    /// The queried person's role in this family.
    pub role: PersonFamilyRole,
    /// The partners' `human_id`s (all partners, including the queried person when they are one).
    pub partners: Vec<String>,
    /// The children: each child's `human_id` and parent relationship.
    pub children: Vec<(String, ChildParentRelationship)>,
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

/// Records a stable external identifier on a family (data-model §11).
///
/// Idempotent in the core: re-adding the same `(authority, value)` emits no event. The resolution
/// key behind re-import (see [`crate::import`]).
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if no such family exists, or a workspace/store error.
pub async fn add_external_id(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    external_id: ExternalId,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    execute(
        store,
        session,
        &family_id.to_string(),
        FamilyCommand::AddExternalId { family_id, external_id },
    )
    .await
}

/// Sets a family's privacy restrictions (GEDCOM `RESN` — data-model §6).
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if no such family exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    execute(
        store,
        session,
        &family_id.to_string(),
        FamilyCommand::SetRestrictions {
            family_id,
            restrictions,
        },
    )
    .await
}

/// Adds a citation backing the family's claims (e.g. a GEDCOM `FAM.SOUR`).
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] / [`AppError::CitationNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn add_family_citation(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    citation_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    let citation_id = resolve_citation_id(store, citation_human_id).await?;
    execute(
        store,
        session,
        &family_id.to_string(),
        FamilyCommand::AddCitation { family_id, citation_id },
    )
    .await
}

/// Attaches a media object to the family (e.g. a GEDCOM `FAM.OBJE`).
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] / [`AppError::MediaNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn attach_family_media(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    media_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    let media_id = resolve_media_id(store, media_human_id).await?;
    let media = MediaRef {
        media_id,
        crop: None,
        caption: None,
        citations: Vec::new(),
    };
    execute(
        store,
        session,
        &family_id.to_string(),
        FamilyCommand::AttachMedia { family_id, media },
    )
    .await
}

/// Attaches a note to the family (e.g. a GEDCOM `FAM.NOTE`).
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] / [`AppError::NoteNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn attach_family_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    let note_id = resolve_note_id(store, note_human_id).await?;
    execute(
        store,
        session,
        &family_id.to_string(),
        FamilyCommand::AttachNote { family_id, note_id },
    )
    .await
}

/// Applies (or, with `remove`, removes) a tag on the family.
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if no such family exists, or a workspace/store error.
pub async fn tag_family(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: TagId,
    remove: bool,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    let command = if remove {
        FamilyCommand::Untag { family_id, tag_id }
    } else {
        FamilyCommand::Tag { family_id, tag_id }
    };
    execute(store, session, &family_id.to_string(), command).await
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

/// Lists the families a person belongs to, with their role in each (partner or child).
///
/// Scans every family for one referencing the person as a partner or a child, resolving member
/// `PersonId`s back to `human_id`s via a single lookup. Returns the families in `human_id` order.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, or a store/read-model error.
pub async fn families_for_person(
    workspace: &Workspace,
    person_human_id: &str,
) -> Result<Vec<FamilyForPerson>, AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, person_human_id).await?;
    let persons: HashMap<PersonId, String> = store
        .list_persons()
        .await?
        .iter()
        .filter_map(|p| Some((p.person_id()?, p.human_id()?.to_string())))
        .collect();
    let resolve = |id: PersonId| persons.get(&id).cloned().unwrap_or_else(|| id.to_string());

    let mut families = Vec::new();
    for view in store.list_families().await? {
        let partner = view.partners().into_iter().any(|id| id == person_id);
        let child_relationship = view
            .children()
            .into_iter()
            .find(|child| child.child_id == person_id)
            .map(|child| child.relationship.clone());
        let role = match (partner, child_relationship) {
            (true, _) => PersonFamilyRole::Partner,
            (false, Some(relationship)) => PersonFamilyRole::Child(relationship),
            (false, None) => continue,
        };
        families.push(FamilyForPerson {
            family_human_id: view.human_id().map(ToString::to_string).unwrap_or_default(),
            role,
            partners: view.partners().into_iter().map(resolve).collect(),
            children: view
                .children()
                .into_iter()
                .map(|child| (resolve(child.child_id), child.relationship.clone()))
                .collect(),
        });
    }
    Ok(families)
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
        .map_err(use_case::map_command_error)
}

/// Resolves a family `human_id` to its aggregate [`FamilyId`], or [`AppError::FamilyNotFound`].
async fn resolve_family_id(store: &Store, human_id: &str) -> Result<FamilyId, AppError> {
    use_case::resolve_id(store.find_family(human_id).await?, FamilyView::family_id, || {
        AppError::FamilyNotFound(human_id.to_owned())
    })
}

/// Resolves a person `human_id` to its aggregate [`PersonId`], or [`AppError::PersonNotFound`].
async fn resolve_person_id(store: &Store, human_id: &str) -> Result<PersonId, AppError> {
    use_case::resolve_id(store.find_person(human_id).await?, PersonView::person_id, || {
        AppError::PersonNotFound(human_id.to_owned())
    })
}

/// Resolves a citation `human_id` to its aggregate [`CitationId`], or [`AppError::CitationNotFound`].
async fn resolve_citation_id(store: &Store, human_id: &str) -> Result<CitationId, AppError> {
    use_case::resolve_id(
        store.find_citation(human_id).await?,
        genealogy_core::citation::CitationView::citation_id,
        || AppError::CitationNotFound(human_id.to_owned()),
    )
}

/// Resolves a media `human_id` to its aggregate [`MediaId`], or [`AppError::MediaNotFound`].
async fn resolve_media_id(store: &Store, human_id: &str) -> Result<MediaId, AppError> {
    use_case::resolve_id(
        store.find_media(human_id).await?,
        genealogy_core::media::MediaView::media_id,
        || AppError::MediaNotFound(human_id.to_owned()),
    )
}

/// Resolves a note `human_id` to its aggregate [`NoteId`], or [`AppError::NoteNotFound`].
async fn resolve_note_id(store: &Store, human_id: &str) -> Result<NoteId, AppError> {
    use_case::resolve_id(
        store.find_note(human_id).await?,
        genealogy_core::note::NoteView::note_id,
        || AppError::NoteNotFound(human_id.to_owned()),
    )
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

    let citation_ids = use_case::citation_human_ids(store).await?;
    let media_ids = use_case::media_human_ids(store).await?;
    let note_ids = use_case::note_human_ids(store).await?;
    let citations = view
        .citations()
        .into_iter()
        .filter_map(|id| citation_ids.get(&id).cloned())
        .collect();
    let media = view
        .media()
        .into_iter()
        .filter_map(|media| media_ids.get(&media.media_id).cloned())
        .collect();
    let notes = view
        .notes()
        .into_iter()
        .filter_map(|id| note_ids.get(&id).cloned())
        .collect();
    let tags = view.tags().into_iter().map(|id| id.to_string()).collect();

    Ok(FamilySummary {
        human_id,
        partners,
        children,
        citations,
        media,
        notes,
        tags,
        restrictions: view.restrictions().clone(),
    })
}
