//! Family use-cases (ADR 0006): create, add/remove partner, add/remove child, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`](genealogy_db::Store),
//! and returns a frontend-neutral [`FamilySummary`] (never a `FamilyView`, cqrs-es, or sqlx type).
//! Partners and children are supplied by Person `human_id` and resolved to a
//! [`PersonId`](genealogy_core::ids::PersonId) here, so the frontend never handles UUIDs. The
//! Family `human_id` is auto-allocated using the workspace's configured format (ADR 0005).

use std::collections::{BTreeSet, HashMap};

use genealogy_core::date::GenealogicalDate;
use genealogy_core::enums::{ChildParentRelationship, EventType, FactType, Restriction};
use genealogy_core::event::EventView;
use genealogy_core::family::FamilyView;
use genealogy_core::family::command::{FamilyCommand, FamilyCommandEnvelope};
use genealogy_core::ids::{AssertionId, CitationId, EventId, FamilyId, HumanId, MediaId, NoteId, PersonId, TagId};
use genealogy_core::person::PersonView;
use genealogy_core::provenance::CitationRef as ProvCitationRef;
use genealogy_core::provenance::Confidence;
use genealogy_core::text::{ExternalId, MediaRef};
use genealogy_db::Store;

use crate::citation::TagRef;
use crate::dto::{AggRef, AttachedRef, CitationRef, MediaRefSummary};
use crate::error::AppError;
use crate::event::{EventSummary, list_events};
use crate::person::list_persons;
use crate::session::Session;
use crate::use_case::{self, MutationMeta, Provenance};
use crate::workspace::Workspace;

/// A family partner, joined to the person projection: their name + lifespan for display, the stable
/// ids for navigation, and the assertion's surety + source count (the evidence-first cue).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartnerRef {
    /// The partner's user-facing identifier (e.g. `I0001`).
    pub human_id: String,
    /// The partner's stable `PersonId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The partner's display name, if resolved.
    pub name: Option<String>,
    /// A "born – died" lifespan summary, if birth/death years are known.
    pub vitals: Option<String>,
    /// The operator's surety in the partnership assertion.
    pub confidence: Confidence,
    /// How many citations back the partnership assertion.
    pub source_count: usize,
    /// The partnership assertion's citations, joined to the source projection — the evidence behind
    /// the partnership, for the provenance popover.
    pub citations: Vec<CitationRef>,
    /// The `AssertionId` (a UUID string) that introduced this partner — the target a per-row Remove
    /// retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// One child-to-partner relationship (GEDCOM `_FREL`/`_MREL`), asserted on its own (ADR 0021), so it
/// carries its own surety, source count, and correction target independent of the child's membership.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRelationshipRef {
    /// The family partner the relationship is to, by `human_id`.
    pub partner_human_id: String,
    /// How the child relates to that partner.
    pub relationship: ChildParentRelationship,
    /// The operator's surety in the relationship assertion.
    pub confidence: Confidence,
    /// How many citations back the relationship assertion.
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) that introduced this link — the target a per-link Edit
    /// supersedes and a clear retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// A family child, joined to the person projection, with one relationship per family partner.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChildRef {
    /// The child's user-facing identifier (e.g. `I0001`).
    pub human_id: String,
    /// The child's stable `PersonId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The child's display name, if resolved.
    pub name: Option<String>,
    /// The child's birth year, if known.
    pub born: Option<String>,
    /// The child's relationship to each family partner, one per-partner assertion (ADR 0021).
    pub relationships: Vec<ChildRelationshipRef>,
    /// The operator's surety in the child's membership assertion.
    pub confidence: Confidence,
    /// How many citations back the child's membership assertion.
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) of the child's membership assertion — the target a per-row
    /// Remove retracts, cascading its relationships (ADR 0004 §2, ADR 0021). Never rendered.
    pub assertion_id: String,
}

/// A family event (e.g. a marriage), joined to the event projection for display.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyEventRef {
    /// The event's user-facing identifier (e.g. `E0001`).
    pub human_id: String,
    /// The event's stable `EventId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The kind of event. Structured so the frontend localizes it (ADR 0003).
    pub event_type: Option<EventType>,
    /// When the event occurred. Structured so the frontend localizes it (ADR 0003).
    pub date: Option<GenealogicalDate>,
    /// The linked place's `human_id`, if any.
    pub place: Option<String>,
    /// The operator's surety in the family-event link.
    pub confidence: Confidence,
    /// How many citations back the event.
    pub source_count: usize,
    /// The linked event's citations, joined to the source projection — the evidence behind the
    /// event, for the provenance popover.
    pub citations: Vec<CitationRef>,
    /// The `AssertionId` (a UUID string) that introduced this family-event link — the target a
    /// per-row Edit supersedes and a Retract retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// A frontend-neutral summary of a family (the DTO the CLI renders). References to other aggregates
/// carry their stable ids alongside their `human_id`s (the cross-aggregate-joins dependency note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilySummary {
    /// The user-facing identifier (e.g. `F0001`).
    pub human_id: String,
    /// The family's stable `FamilyId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The partners (neutral roles), joined to the person projection.
    pub partners: Vec<PartnerRef>,
    /// The children, joined to the person projection, with per-partner relationships.
    pub children: Vec<ChildRef>,
    /// The linked family events (e.g. a marriage), joined to the event projection.
    pub events: Vec<FamilyEventRef>,
    /// Citations backing the family's claims (e.g. `FAM.SOUR`), in assertion order.
    pub citations: Vec<AggRef>,
    /// Media attached to the family (e.g. `FAM.OBJE`), in assertion order.
    pub media: Vec<MediaRefSummary>,
    /// Notes attached to the family (e.g. `FAM.NOTE`), with the attach `AssertionId` (the Detach
    /// target), in assertion order.
    pub notes: Vec<AttachedRef>,
    /// Tags applied to the family, by name + colour (never by id — data-model §9).
    pub tags: Vec<TagRef>,
    /// The family's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// A person's role within a family: a partner/spouse, or a child (with the per-partner relationships).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonFamilyRole {
    /// The person is a partner/spouse in the family.
    Partner,
    /// The person is a child in the family, with the recorded relationship to each family partner.
    Child(Vec<(String, ChildParentRelationship)>),
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
    /// The children: each child's `human_id` and its relationship to each family partner (by partner
    /// `human_id`).
    pub children: Vec<(String, Vec<(String, ChildParentRelationship)>)>,
}

/// Creates a family, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::FamilyDomain`] if a domain rule rejects the command, or a workspace/store error.
pub async fn create_family(
    workspace: &Workspace,
    session: &Session,
    provenance: Provenance,
    citations: &[String],
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = store.next_family_human_id(&workspace.family_id_format()?).await?;
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;

    let family_id = session.new_family_id();
    execute(
        store,
        session,
        &family_id.to_string(),
        FamilyCommand::CreateFamily {
            family_id,
            human_id: HumanId::new(&human_id),
        },
        provenance,
        citation_refs,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, family_human_id).await?;
    let person_id = resolve_person_id(store, person_human_id).await?;
    execute_family_mutation(
        store,
        session,
        family_id,
        FamilyCommand::AddPartner { family_id, person_id },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, family_human_id).await?;
    let person_id = resolve_person_id(store, person_human_id).await?;
    execute_family_mutation(
        store,
        session,
        family_id,
        FamilyCommand::RemovePartner { family_id, person_id },
        meta,
    )
    .await
}

/// Adds a child (by person `human_id`) to the family, with one relationship per family partner.
///
/// `relationships` pairs a partner's `human_id` with the child's relationship to that partner
/// (GEDCOM `_FREL`/`_MREL`); each partner `human_id` is resolved to its `PersonId`.
///
/// # Errors
///
/// As [`add_partner`], but rejects with [`AppError::FamilyDomain`] if the child is already present,
/// or [`AppError::PersonNotFound`] if a referenced partner does not exist.
pub async fn add_child(
    workspace: &Workspace,
    session: &Session,
    family_human_id: &str,
    child_human_id: &str,
    relationships: Vec<(String, ChildParentRelationship)>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    // Validate every reference first, so a bad partner id fails before any event is written.
    let family_id = resolve_family_id(store, family_human_id).await?;
    let child_id = resolve_person_id(store, child_human_id).await?;
    let mut resolved = Vec::with_capacity(relationships.len());
    for (partner_human_id, relationship) in &relationships {
        let partner_id = resolve_person_id(store, partner_human_id).await?;
        resolved.push((partner_id, relationship.clone()));
    }
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    // Membership first, then one assertion per parent link (ADR 0021). cqrs-es commits per
    // aggregate, so a failure after membership leaves an orphaned membership row — recoverable by
    // re-running, never corruption.
    execute(
        store,
        session,
        &family_id.to_string(),
        FamilyCommand::AddChild { family_id, child_id },
        meta.provenance.clone(),
        citations.clone(),
    )
    .await?;
    for (parent_id, relationship) in resolved {
        execute(
            store,
            session,
            &family_id.to_string(),
            FamilyCommand::AssertChildRelationship {
                family_id,
                child_id,
                parent_id,
                relationship,
            },
            meta.provenance.clone(),
            citations.clone(),
        )
        .await?;
    }
    Ok(())
}

/// Asserts one child-to-partner relationship (GEDCOM `_FREL`/`_MREL` — ADR 0021), the per-link edit
/// path: a `meta.supersedes` replaces the prior link's assertion, so an adoption link is corrected
/// without touching the child's membership or the other links.
///
/// # Errors
///
/// [`AppError::FamilyNotFound`]/[`AppError::PersonNotFound`] if the family, child, or partner does
/// not exist, [`AppError::FamilyDomain`] if the child is not a member, the partner is not a current
/// partner, or the `(child, partner)` link is already present, or a workspace/store error.
pub async fn assert_child_relationship(
    workspace: &Workspace,
    session: &Session,
    family_human_id: &str,
    child_human_id: &str,
    partner_human_id: &str,
    relationship: ChildParentRelationship,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, family_human_id).await?;
    let child_id = resolve_person_id(store, child_human_id).await?;
    let parent_id = resolve_person_id(store, partner_human_id).await?;
    execute_family_mutation(
        store,
        session,
        family_id,
        FamilyCommand::AssertChildRelationship {
            family_id,
            child_id,
            parent_id,
            relationship,
        },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, family_human_id).await?;
    let child_id = resolve_person_id(store, child_human_id).await?;
    execute_family_mutation(
        store,
        session,
        family_id,
        FamilyCommand::RemoveChild { family_id, child_id },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    execute_family_mutation(
        store,
        session,
        family_id,
        FamilyCommand::AddExternalId { family_id, external_id },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    execute_family_mutation(
        store,
        session,
        family_id,
        FamilyCommand::SetRestrictions {
            family_id,
            restrictions,
        },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    let citation_id = resolve_citation_id(store, citation_human_id).await?;
    execute_family_mutation(
        store,
        session,
        family_id,
        FamilyCommand::AddCitation { family_id, citation_id },
        meta,
    )
    .await
}

/// Links a family event (an `Event` aggregate, e.g. a marriage — `FAM.MARR`) to the family.
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] / [`AppError::EventNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn link_family_event(
    workspace: &Workspace,
    session: &Session,
    family_human_id: &str,
    event_human_id: &str,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, family_human_id).await?;
    let event_id = resolve_event_id(store, event_human_id).await?;
    execute_family_mutation(
        store,
        session,
        family_id,
        FamilyCommand::LinkFamilyEvent { family_id, event_id },
        meta,
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
    meta: MutationMeta<'_>,
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
    execute_family_mutation(
        store,
        session,
        family_id,
        FamilyCommand::AttachMedia { family_id, media },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    let note_id = resolve_note_id(store, note_human_id).await?;
    execute_family_mutation(
        store,
        session,
        family_id,
        FamilyCommand::AttachNote { family_id, note_id },
        meta,
    )
    .await
}

/// Applies (or, with `remove`, removes) a tag on the family.
///
/// The `tag_id` is a tag's aggregate id (a UUID string), resolved from a tag the user picked by
/// name; it is never shown to the user (data-model §9). Mirrors [`tag_citation`](crate::tag_citation).
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if no such family exists, [`AppError::TagNotFound`] if `tag_id` is
/// not a valid id, or a workspace/store error.
pub async fn tag_family(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: &str,
    remove: bool,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    let command = if remove {
        FamilyCommand::Untag { family_id, tag_id }
    } else {
        FamilyCommand::Tag { family_id, tag_id }
    };
    execute_family_mutation(store, session, family_id, command, meta).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
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
    let lookups = FamilyLookups::load(workspace).await?;
    Ok(Some(summarize(&view, &lookups)))
}

/// Lists every family's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_families(workspace: &Workspace) -> Result<Vec<FamilySummary>, AppError> {
    let store = workspace.store();
    let views = store.list_families().await?;
    let lookups = FamilyLookups::load(workspace).await?;
    Ok(views.iter().map(|view| summarize(view, &lookups)).collect())
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

    // Maps a child's per-`PersonId` relationships to per-partner-`human_id` relationships.
    let resolve_relationships = |relationships: &[(PersonId, ChildParentRelationship)]| {
        relationships
            .iter()
            .map(|(partner_id, relationship)| (resolve(*partner_id), relationship.clone()))
            .collect::<Vec<_>>()
    };

    let mut families = Vec::new();
    for view in store.list_families().await? {
        let partner = view.partners().into_iter().any(|id| id == person_id);
        let child_relationships = view
            .children()
            .into_iter()
            .find(|child| child.child_id == person_id)
            .map(|child| resolve_relationships(&child.relationships));
        let role = match (partner, child_relationships) {
            (true, _) => PersonFamilyRole::Partner,
            (false, Some(relationships)) => PersonFamilyRole::Child(relationships),
            (false, None) => continue,
        };
        families.push(FamilyForPerson {
            family_human_id: view.human_id().map(ToString::to_string).unwrap_or_default(),
            role,
            partners: view.partners().into_iter().map(resolve).collect(),
            children: view
                .children()
                .into_iter()
                .map(|child| (resolve(child.child_id), resolve_relationships(&child.relationships)))
                .collect(),
        });
    }
    Ok(families)
}

/// Sets (or changes) a family's user-facing identifier, identified by its current `human_id`,
/// returning the effective new id.
///
/// A supplied non-blank `new` id is dup-checked (a collision with a *different* record is
/// [`AppError::HumanIdTaken`]); a blank/absent `new` allocates the next free id from the workspace's
/// configured format (the regenerate case).
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if the family is unknown, [`AppError::HumanIdTaken`] if the requested
/// id is already in use, or a workspace/store error.
pub async fn set_family_human_id(
    workspace: &Workspace,
    session: &Session,
    current_human_id: &str,
    new: Option<String>,
    provenance: Provenance,
) -> Result<String, AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, current_human_id).await?;
    let human_id = match use_case::requested_human_id(new) {
        Some(id) => {
            if id != current_human_id && store.find_family(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_family_human_id(&workspace.family_id_format()?).await?,
    };
    execute(
        store,
        session,
        &family_id.to_string(),
        FamilyCommand::SetHumanId {
            family_id,
            human_id: HumanId::new(&human_id),
        },
        provenance,
        Vec::new(),
    )
    .await?;
    Ok(human_id)
}

/// Executes one command through the store, stamping the operator `provenance` and backing
/// `citations`, and mapping the command outcome to [`AppError`].
async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: FamilyCommand,
    provenance: Provenance,
    citations: Vec<ProvCitationRef>,
) -> Result<(), AppError> {
    let envelope = FamilyCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_family(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Executes one non-create family mutation, applying the operator-intent [`MutationMeta`]: resolves
/// the backing citations, and — when `meta.supersedes` is set — wraps `command` in a
/// [`FamilyCommand::SupersedeAssertion`] so the new assertion replaces the named one (ADR 0004 §2).
async fn execute_family_mutation(
    store: &Store,
    session: &Session,
    family_id: FamilyId,
    command: FamilyCommand,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    let target = use_case::parse_supersedes(meta.supersedes)?;
    let command = superseded(family_id, command, target);
    execute(
        store,
        session,
        &family_id.to_string(),
        command,
        meta.provenance,
        citations,
    )
    .await
}

/// Wraps `command` in a [`FamilyCommand::SupersedeAssertion`] against `target` when superseding, or
/// returns it unchanged for a plain assertion.
fn superseded(family_id: FamilyId, command: FamilyCommand, target: Option<AssertionId>) -> FamilyCommand {
    match target {
        Some(target) => FamilyCommand::SupersedeAssertion {
            family_id,
            target,
            replacement: Box::new(command),
        },
        None => command,
    }
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

/// Resolves an event `human_id` to its aggregate [`EventId`], or [`AppError::EventNotFound`].
async fn resolve_event_id(store: &Store, human_id: &str) -> Result<EventId, AppError> {
    use_case::resolve_id(store.find_event(human_id).await?, EventView::event_id, || {
        AppError::EventNotFound(human_id.to_owned())
    })
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

/// A person joined to the Person projection: the `human_id`, display name, and lifespan years.
struct PersonInfo {
    human_id: String,
    name: Option<String>,
    birth_year: Option<i32>,
    death_year: Option<i32>,
}

/// An event joined to the Event projection: the `human_id`, kind, date, place, and source count.
struct EventInfo {
    human_id: String,
    event_type: Option<EventType>,
    date: Option<GenealogicalDate>,
    place: Option<String>,
    source_count: usize,
    citations: Vec<CitationRef>,
}

/// The lookups `summarize` needs to join a family's members and attachments to the other
/// projections without a per-row query (the cross-aggregate join lives here — the app/db layer).
struct FamilyLookups {
    persons: HashMap<PersonId, PersonInfo>,
    events: HashMap<EventId, EventInfo>,
    citations: HashMap<CitationId, CitationRef>,
    media: HashMap<MediaId, String>,
    notes: HashMap<NoteId, String>,
    tags: HashMap<TagId, TagRef>,
}

impl FamilyLookups {
    async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let store = workspace.store();
        let person_ids: HashMap<String, PersonId> = store
            .list_persons()
            .await?
            .iter()
            .filter_map(|p| Some((p.human_id()?.to_string(), p.person_id()?)))
            .collect();
        let mut persons = HashMap::new();
        for summary in list_persons(workspace).await? {
            if let Some(id) = person_ids.get(&summary.human_id) {
                persons.insert(
                    *id,
                    PersonInfo {
                        human_id: summary.human_id.clone(),
                        name: summary.display_name.clone(),
                        birth_year: crate::dto::year_of_fact(&summary, &FactType::Birth),
                        death_year: crate::dto::year_of_fact(&summary, &FactType::Death),
                    },
                );
            }
        }

        let event_ids: HashMap<String, EventId> = store
            .list_events()
            .await?
            .iter()
            .filter_map(|e| Some((e.human_id()?.to_string(), e.event_id()?)))
            .collect();
        let mut events = HashMap::new();
        for summary in list_events(workspace).await? {
            if let Some(id) = event_ids.get(&summary.human_id) {
                events.insert(*id, event_info(summary));
            }
        }

        Ok(Self {
            persons,
            events,
            citations: crate::dto::citation_refs(store).await?,
            media: use_case::media_human_ids(store).await?,
            notes: use_case::note_human_ids(store).await?,
            tags: tag_labels(store).await?,
        })
    }
}

/// Builds an [`EventInfo`] from a resolved [`EventSummary`].
fn event_info(summary: EventSummary) -> EventInfo {
    EventInfo {
        human_id: summary.human_id,
        event_type: summary.event_type,
        date: summary.date,
        place: summary.place.map(|p| p.name.unwrap_or(p.human_id)),
        source_count: summary.citations.len(),
        citations: summary.citations,
    }
}

/// Builds a `TagId -> TagRef` lookup from the Tag projection, to render applied tags by name/colour/
/// priority (never by id — data-model §9).
async fn tag_labels(store: &Store) -> Result<HashMap<TagId, TagRef>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_tags().await? {
        if let (Some(id), Some(name)) = (view.tag_id(), view.name()) {
            map.insert(
                id,
                TagRef {
                    id: id.to_string(),
                    name: name.to_owned(),
                    color: view.color().map(ToOwned::to_owned),
                    priority: view.priority(),
                },
            );
        }
    }
    Ok(map)
}

/// Renders a [`FamilyView`] into the frontend DTO, joining members and attachments to the other
/// projections via `lookups`.
///
/// A member whose projection is missing (a dangling reference) renders with its UUID as the
/// `human_id` and no joined detail, rather than failing the whole read.
fn summarize(view: &FamilyView, lookups: &FamilyLookups) -> FamilySummary {
    let partners = summarize_partners(view, lookups);
    let children = summarize_children(view, lookups);
    let events = summarize_events(view, lookups);

    let citations = view
        .citations()
        .into_iter()
        .filter_map(|id| {
            lookups.citations.get(&id).map(|citation| AggRef {
                human_id: citation.human_id.clone(),
                id: citation.id.clone(),
            })
        })
        .collect();
    let media = view
        .media_with_assertions()
        .iter()
        .filter_map(|attributed| {
            let media = &attributed.value;
            lookups.media.get(&media.media_id).map(|human_id| MediaRefSummary {
                human_id: human_id.clone(),
                id: media.media_id.to_string(),
                caption: media.caption.clone(),
                assertion_id: attributed.assertion_id.to_string(),
            })
        })
        .collect();
    let notes = view
        .notes_with_assertions()
        .iter()
        .filter_map(|attributed| {
            lookups.notes.get(&attributed.value).map(|human_id| AttachedRef {
                human_id: human_id.clone(),
                id: attributed.value.to_string(),
                assertion_id: attributed.assertion_id.to_string(),
            })
        })
        .collect();
    let tags = view
        .tags()
        .into_iter()
        .filter_map(|id| lookups.tags.get(&id).cloned())
        .collect();

    FamilySummary {
        human_id: view.human_id().map(ToString::to_string).unwrap_or_default(),
        id: view.family_id().map(|id| id.to_string()).unwrap_or_default(),
        partners,
        children,
        events,
        citations,
        media,
        notes,
        tags,
        restrictions: view.restrictions().clone(),
    }
}

/// Joins each family partner to the person projection (name, lifespan, stable id) with its surety.
fn summarize_partners(view: &FamilyView, lookups: &FamilyLookups) -> Vec<PartnerRef> {
    view.partners_with_assertions()
        .iter()
        .map(|attributed| {
            let partner = &attributed.value;
            let info = lookups.persons.get(&partner.person_id);
            PartnerRef {
                human_id: info.map_or_else(|| partner.person_id.to_string(), |i| i.human_id.clone()),
                id: partner.person_id.to_string(),
                name: info.and_then(|i| i.name.clone()),
                vitals: info.and_then(|i| crate::dto::lifespan(i.birth_year, i.death_year)),
                confidence: partner.confidence,
                source_count: partner.citations.len(),
                citations: partner
                    .citations
                    .iter()
                    .filter_map(|id| lookups.citations.get(id).cloned())
                    .collect(),
                assertion_id: attributed.assertion_id.to_string(),
            }
        })
        .collect()
}

/// Joins each family child to the person projection, folding its per-partner relationship rows
/// (ADR 0021) into per-partner-`human_id` links, each carrying its own surety + source + assertion id.
fn summarize_children(view: &FamilyView, lookups: &FamilyLookups) -> Vec<ChildRef> {
    let resolve_partner_human = |partner_id: PersonId| {
        lookups
            .persons
            .get(&partner_id)
            .map_or_else(|| partner_id.to_string(), |i| i.human_id.clone())
    };
    let links = view.child_relationships_with_assertions();
    view.children_with_assertions()
        .iter()
        .map(|attributed| {
            let child = &attributed.value;
            let info = lookups.persons.get(&child.child_id);
            let relationships = links
                .iter()
                .filter(|link| link.value.value.child_id == child.child_id)
                .map(|link| ChildRelationshipRef {
                    partner_human_id: resolve_partner_human(link.value.value.parent_id),
                    relationship: link.value.value.relationship.clone(),
                    confidence: link.value.confidence,
                    source_count: link.value.citations.len(),
                    assertion_id: link.assertion_id.to_string(),
                })
                .collect();
            ChildRef {
                human_id: info.map_or_else(|| child.child_id.to_string(), |i| i.human_id.clone()),
                id: child.child_id.to_string(),
                name: info.and_then(|i| i.name.clone()),
                born: info.and_then(|i| i.birth_year).map(|year| year.to_string()),
                relationships,
                confidence: child.confidence,
                source_count: child.citations.len(),
                assertion_id: attributed.assertion_id.to_string(),
            }
        })
        .collect()
}

/// Joins each linked family event to the event projection (kind, date, place, source count).
fn summarize_events(view: &FamilyView, lookups: &FamilyLookups) -> Vec<FamilyEventRef> {
    view.linked_events_with_assertions()
        .iter()
        .map(|attributed| {
            let linked = &attributed.value;
            let info = lookups.events.get(&linked.event_id);
            FamilyEventRef {
                human_id: info.map_or_else(|| linked.event_id.to_string(), |i| i.human_id.clone()),
                id: linked.event_id.to_string(),
                event_type: info.and_then(|i| i.event_type.clone()),
                date: info.and_then(|i| i.date.clone()),
                place: info.and_then(|i| i.place.clone()),
                confidence: linked.confidence,
                source_count: info.map_or(0, |i| i.source_count),
                citations: info.map_or_else(Vec::new, |i| i.citations.clone()),
                assertion_id: attributed.assertion_id.to_string(),
            }
        })
        .collect()
}
