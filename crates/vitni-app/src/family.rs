//! Family use-cases (ADR 0006): create, add/remove partner, add/remove child, show, and list.
//!
//! Each builds a command + [`AssertionMeta`](vitni_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`](vitni_db::Store),
//! and returns a frontend-neutral [`FamilySummary`] (never a `FamilyView`, cqrs-es, or sqlx type).
//! Partners and children are supplied by Person `human_id` and resolved to a
//! [`PersonId`](vitni_core::ids::PersonId) here, so the frontend never handles UUIDs. The
//! Family `human_id` is auto-allocated using the workspace's configured format (ADR 0005).

use std::collections::{BTreeMap, BTreeSet, HashMap};

use vitni_core::date::GenealogicalDate;
use vitni_core::enums::{ChildParentRelationship, EventType, Restriction};
use vitni_core::event::EventView;
use vitni_core::family::command::{FamilyCommand, FamilyCommandEnvelope};
use vitni_core::family::error::FamilyError;
use vitni_core::family::{ChildEntry, FamilyView};
use vitni_core::ids::{AssertionId, CitationId, EventId, FamilyId, HumanId, MediaId, NoteId, PersonId, TagId};
use vitni_core::person::PersonView;
use vitni_core::provenance::Confidence;
use vitni_core::provenance::EvidenceRef;
use vitni_core::text::{ExternalId, MediaRef};
use vitni_db::Store;

use crate::citation::TagRef;
use crate::dto::{AggRef, AttachedRef, CitationRef, MediaLookup, MediaRefSummary};
use crate::error::AppError;
use crate::event::{EventSummary, list_events};
use crate::identity::{self, EventClusters, FamilyClusters, IdentityDecision, PairDecision, PersonClusters};
use crate::person::{list_persons, render_name};
use crate::session::Session;
use crate::use_case::{self, MediaRefInput, MutationMeta, Provenance};
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
    pub confidence: Option<Confidence>,
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
    pub confidence: Option<Confidence>,
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
    pub confidence: Option<Confidence>,
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
    pub confidence: Option<Confidence>,
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
    /// Citations backing the family's claims (e.g. `FAM.SOUR`), joined to the source projection, each
    /// carrying its attach `AssertionId` (the Detach target), in assertion order.
    pub citations: Vec<CitationRef>,
    /// Media attached to the family (e.g. `FAM.OBJE`), in assertion order.
    pub media: Vec<MediaRefSummary>,
    /// Notes attached to the family (e.g. `FAM.NOTE`), with the attach `AssertionId` (the Detach
    /// target), in assertion order.
    pub notes: Vec<AttachedRef>,
    /// Tags applied to the family, by name + colour (never by id — data-model §9).
    pub tags: Vec<TagRef>,
    /// The family's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
    /// Every family merged into this family's cluster, directly or through another member (ADR 0039
    /// §4), in id order.
    pub merged: Vec<AggRef>,
    /// The `human_id` of the member each member-owned row came from, by the row's `AssertionId` — the
    /// stream an edit or retraction of that row is written to (ADR 0039 §5). A row absent here is the
    /// family's own. Never rendered as a key; see [`owner_of`](Self::owner_of).
    pub claim_owners: BTreeMap<String, String>,
}

impl FamilySummary {
    /// The `human_id` of the record that owns the row introduced by `assertion_id`: the member it came
    /// from, or this family for its own rows.
    #[must_use]
    pub fn owner_of(&self, assertion_id: &str) -> &str {
        self.claim_owners
            .get(assertion_id)
            .map_or(self.human_id.as_str(), String::as_str)
    }
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
    create_family_with_external_ids(workspace, session, Vec::new(), provenance, citations).await
}

/// Creates a family carrying `external_ids`, so the family and its keys commit together
/// (data-model §11) — what [`crate::import::import_family`] needs to never leave a keyless family.
///
/// # Errors
///
/// As [`create_family`].
pub(crate) async fn create_family_with_external_ids(
    workspace: &Workspace,
    session: &Session,
    external_ids: Vec<ExternalId>,
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
            external_ids: use_case::attribute_each(session, external_ids),
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

/// Removes a child (by person `human_id`) from the family — from every record of the family's
/// cluster that names the child, or any record of the child's own cluster (ADR 0039 §5).
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
    let persons = PersonClusters::load(store).await?;
    let mut holders = Vec::new();
    for record in identity::cluster_records(store, family_id).await? {
        let Some(id) = record.family_id() else { continue };
        for child in record.children() {
            if persons.root(child.child_id) == persons.root(child_id) {
                holders.push((id, child.child_id));
            }
        }
    }
    let Some((&(first, first_child), rest)) = holders.split_first() else {
        let command = FamilyCommand::RemoveChild { family_id, child_id };
        return execute_family_mutation(store, session, family_id, command, meta).await;
    };
    let citations = use_case::resolve_evidence_refs(store, meta.citations, meta.dna_matches).await?;
    for &(id, child_id) in rest {
        let command = FamilyCommand::RemoveChild {
            family_id: id,
            child_id,
        };
        execute(
            store,
            session,
            &id.to_string(),
            command,
            meta.provenance.clone(),
            citations.clone(),
        )
        .await?;
    }
    let command = FamilyCommand::RemoveChild {
        family_id: first,
        child_id: first_child,
    };
    execute(store, session, &first.to_string(), command, meta.provenance, citations).await
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

/// Sets a family's privacy restrictions (GEDCOM `RESN` — data-model §6). A merged member restricted
/// beyond the new set is narrowed to it first, so the cluster reads with exactly `restrictions`
/// (ADR 0039 §5).
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
    let citations = use_case::resolve_evidence_refs(store, meta.citations, meta.dna_matches).await?;
    for record in identity::cluster_records(store, family_id).await? {
        let Some(id) = record.family_id() else { continue };
        if id == family_id || record.restrictions().is_subset(&restrictions) {
            continue;
        }
        let narrowed = record.restrictions().intersection(&restrictions).copied().collect();
        let command = FamilyCommand::SetRestrictions {
            family_id: id,
            restrictions: narrowed,
        };
        execute(
            store,
            session,
            &id.to_string(),
            command,
            meta.provenance.clone(),
            citations.clone(),
        )
        .await?;
    }
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
    input: MediaRefInput,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let family_id = resolve_family_id(store, human_id).await?;
    let media_id = resolve_media_id(store, media_human_id).await?;
    let media = MediaRef {
        media_id,
        crop: input.crop,
        caption: input.caption,
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

/// Re-edits an existing family media attachment (crop / caption) by the `AssertionId` of the attach
/// assertion — supersedes it with a new `MediaAttached` carrying the same media and citations plus
/// the new crop/caption (the row-Edit correction, ADR 0004 §2).
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if no such family exists, [`AppError::FamilyDomain`] if
/// `assertion_id` names no live media attachment, or a workspace/store error.
pub async fn update_family_media_ref(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    assertion_id: &str,
    input: MediaRefInput,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let view = store
        .find_family(human_id)
        .await?
        .ok_or_else(|| AppError::FamilyNotFound(human_id.to_owned()))?;
    let family_id = resolve_family_id(store, human_id).await?;
    let target = use_case::parse_assertion_id(assertion_id)?;
    let existing = view
        .media_with_assertions()
        .iter()
        .find(|attributed| attributed.assertion_id == target)
        .ok_or(AppError::FamilyDomain(FamilyError::SupersedesMissingAssertion(target)))?;
    let media = MediaRef {
        media_id: existing.value.media_id,
        crop: input.crop,
        caption: input.caption,
        citations: existing.value.citations.clone(),
    };
    let meta = MutationMeta {
        supersedes: Some(assertion_id),
        ..meta
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

/// Applies (or, with `remove`, removes) a tag on the family. A removed tag is untagged on every
/// record of the family's cluster that holds it (ADR 0039 §5).
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
    if !remove {
        let command = FamilyCommand::Tag { family_id, tag_id };
        return execute_family_mutation(store, session, family_id, command, meta).await;
    }
    let citations = use_case::resolve_evidence_refs(store, meta.citations, meta.dna_matches).await?;
    for record in identity::cluster_records(store, family_id).await? {
        let Some(id) = record.family_id() else { continue };
        if id != family_id && record.tags().contains(&tag_id) {
            let command = FamilyCommand::Untag { family_id: id, tag_id };
            execute(
                store,
                session,
                &id.to_string(),
                command,
                meta.provenance.clone(),
                citations.clone(),
            )
            .await?;
        }
    }
    let command = FamilyCommand::Untag { family_id, tag_id };
    execute_family_mutation(store, session, family_id, command, meta).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// The outcome of [`merge_families`]: the survivor's refreshed summary and the merged family's
/// `human_id`.
///
/// The merge is a same-as link on the survivor (ADR 0039 §1): no record that names the merged family
/// is rewritten. Every reader resolves those references to the cluster's root instead (ADR 0039 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyMergeResult {
    /// The survivor's summary after the merge, composed with every record now in its cluster.
    pub survivor: FamilySummary,
    /// The merged family's `human_id` (its own record/stream is untouched).
    pub merged_human_id: String,
}

/// Merges `merged_human_id`'s cluster into `surviving_human_id`'s, recording a same-as link
/// (ADR 0039 §1). Both records resolve to their cluster roots first (§4), and one `FamiliesMerged`
/// event is emitted on the surviving root's stream, carrying the decision's provenance and assessment.
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if either `human_id` does not resolve; [`AppError::FamilyDomain`] with
/// `MergeConflict` if they resolve to the same family, or `IdentityDecided` if the two are already one
/// cluster or a record of one cluster is distinguished from a record of the other; or a store error.
pub async fn merge_families(
    workspace: &Workspace,
    session: &Session,
    surviving_human_id: &str,
    merged_human_id: &str,
    decision: IdentityDecision,
) -> Result<FamilyMergeResult, AppError> {
    let pair = identity::merge::<FamilyView>(workspace, session, surviving_human_id, merged_human_id, decision).await?;
    family_merge_result(workspace, &pair.first_human_id, merged_human_id).await
}

/// Undoes every live distinction between the two family clusters, then merges them (ADR 0039 §4).
///
/// # Errors
///
/// As [`merge_families`], except that a distinction between the clusters no longer refuses the merge.
pub async fn undo_family_distinction_and_merge(
    workspace: &Workspace,
    session: &Session,
    surviving_human_id: &str,
    merged_human_id: &str,
    decision: IdentityDecision,
) -> Result<FamilyMergeResult, AppError> {
    let pair = identity::undo_distinction_and_merge::<FamilyView>(
        workspace,
        session,
        surviving_human_id,
        merged_human_id,
        decision,
    )
    .await?;
    family_merge_result(workspace, &pair.first_human_id, merged_human_id).await
}

/// The survivor's composed summary after a merge.
async fn family_merge_result(
    workspace: &Workspace,
    survivor_human_id: &str,
    merged_human_id: &str,
) -> Result<FamilyMergeResult, AppError> {
    let survivor = show_family(workspace, survivor_human_id)
        .await?
        .ok_or_else(|| AppError::FamilyNotFound(survivor_human_id.to_owned()))?;
    Ok(FamilyMergeResult {
        survivor,
        merged_human_id: merged_human_id.to_owned(),
    })
}

/// Records that the two families are different (ADR 0039 §1), so neither cluster is proposed as a
/// duplicate of the other again. One `FamiliesDistinguished` event is emitted on the first root's
/// stream; undoing it lifts the decision.
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if either `human_id` does not resolve; [`AppError::FamilyDomain`] with
/// `DistinctFromItself` if they resolve to the same family, or `IdentityDecided` if the two are already
/// one cluster or already distinguished; or a store error.
pub async fn distinguish_families(
    workspace: &Workspace,
    session: &Session,
    family_human_id: &str,
    other_human_id: &str,
    decision: IdentityDecision,
) -> Result<(), AppError> {
    identity::distinguish::<FamilyView>(workspace, session, family_human_id, other_human_id, decision).await
}

/// The live identity decision between the clusters of two families, or `None` when the pair is
/// undecided (ADR 0039 §4).
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if either `human_id` does not resolve, or a store error.
pub async fn family_pair_decision(
    workspace: &Workspace,
    first_human_id: &str,
    other_human_id: &str,
) -> Result<Option<PairDecision>, AppError> {
    identity::pair_decision::<FamilyView>(workspace, first_human_id, other_human_id).await
}

/// The `human_id` of the record of `human_id`'s family cluster whose stream holds the live assertion
/// `assertion_id` — where an edit or retraction of that row is written (ADR 0039 §5).
///
/// # Errors
///
/// [`AppError::FamilyNotFound`] if `human_id` is unknown, [`AppError::Db`] if `assertion_id` is not a
/// UUID, or a store error.
pub async fn family_claim_owner(workspace: &Workspace, human_id: &str, assertion_id: &str) -> Result<String, AppError> {
    identity::claim_owner::<FamilyView>(workspace, human_id, assertion_id).await
}

/// Loads a single family's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_family(workspace: &Workspace, human_id: &str) -> Result<Option<FamilySummary>, AppError> {
    let store = workspace.store();
    let Some(found) = store.find_family(human_id).await? else {
        return Ok(None);
    };
    let Some(family_id) = found.family_id() else {
        return Ok(None);
    };
    let clusters = FamilyClusters::load(store).await?;
    let views = identity::views(store, &clusters.cluster(clusters.root(family_id))).await?;
    let lookups = FamilyLookups::load(workspace).await?;
    Ok(summarize_cluster(&views.iter().collect::<Vec<_>>(), &lookups))
}

/// Lists every family's summary, ordered by `human_id`. A merged family is listed once, as its
/// cluster's root (ADR 0039 §5).
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_families(workspace: &Workspace) -> Result<Vec<FamilySummary>, AppError> {
    let store = workspace.store();
    let views = store.list_families().await?;
    let clusters = FamilyClusters::load(store).await?;
    let lookups = FamilyLookups::load(workspace).await?;
    let mut summaries = Vec::with_capacity(views.len());
    for cluster in root_clusters(&views, &clusters) {
        summaries.extend(summarize_cluster(&cluster, &lookups));
    }
    Ok(summaries)
}

/// Groups `views` into their clusters, root first, one per root in `views` order; a member never
/// starts a group of its own (ADR 0039 §5).
pub(crate) fn root_clusters<'a>(views: &'a [FamilyView], clusters: &FamilyClusters) -> Vec<Vec<&'a FamilyView>> {
    let by_id: HashMap<FamilyId, &FamilyView> = views
        .iter()
        .filter_map(|view| Some((view.family_id()?, view)))
        .collect();
    let mut groups = Vec::new();
    for view in views {
        let Some(id) = view.family_id() else { continue };
        if clusters.is_member(id) {
            continue;
        }
        groups.push(
            clusters
                .cluster(id)
                .iter()
                .filter_map(|id| by_id.get(id).copied())
                .collect(),
        );
    }
    groups
}

/// A partner on a lightweight family list row: the partner's user-facing id and display name.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyPartnerRow {
    /// The partner's user-facing identifier (e.g. `I0001`), or the raw id when unresolved.
    pub human_id: String,
    /// The partner's display name, if the person has a name.
    pub name: Option<String>,
}

/// A lightweight family list row: the partners, the marriage date (if any), and the child count —
/// only the fields a list view renders.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyRow {
    /// The user-facing identifier (e.g. `F0001`).
    pub human_id: String,
    /// The family's partners, in assertion order (drives the row title).
    pub partners: Vec<FamilyPartnerRow>,
    /// The date of the family's first linked Marriage event, if that event carries one.
    pub marriage_date: Option<GenealogicalDate>,
    /// How many children the family has (drives the row subtitle).
    pub child_count: usize,
}

/// Lists every family as a lightweight [`FamilyRow`], ordered by `human_id`.
///
/// Unlike [`list_families`], this builds narrow `PersonId -> name` and `EventId -> (type, date)` maps
/// straight from the Person and Event projections, then reads the Family projection — skipping
/// [`FamilyLookups::load`], which re-runs the full `list_persons` **and** `list_events` summary
/// pipelines to join data a list row never shows. Opening a family still uses [`show_family`].
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_family_rows(workspace: &Workspace) -> Result<Vec<FamilyRow>, AppError> {
    let store = workspace.store();
    let clusters = PersonClusters::load(store).await?;
    let event_clusters = EventClusters::load(store).await?;
    let mut partners: HashMap<PersonId, FamilyPartnerRow> = HashMap::new();
    for view in store.list_persons().await? {
        if let (Some(id), Some(human_id)) = (view.person_id(), view.human_id()) {
            partners.insert(
                id,
                FamilyPartnerRow {
                    human_id: human_id.as_str().to_owned(),
                    name: view.names().first().copied().map(render_name),
                },
            );
        }
    }
    let mut events: HashMap<EventId, (Option<EventType>, Option<GenealogicalDate>)> = HashMap::new();
    for view in store.list_events().await? {
        if let Some(id) = view.event_id() {
            events.insert(id, (view.event_type().cloned(), view.date().cloned()));
        }
    }
    let views = store.list_families().await?;
    let family_clusters = FamilyClusters::load(store).await?;
    let mut rows = Vec::with_capacity(views.len());
    for cluster in root_clusters(&views, &family_clusters) {
        let Some(root) = cluster.first() else { continue };
        let partner_ids = cluster_partners(&cluster, &clusters);
        let child_count = cluster_children(&cluster, &clusters).len();
        let mut marriage_date = None;
        for view in &cluster {
            marriage_date = marriage_date.or_else(|| {
                view.linked_events()
                    .into_iter()
                    .find_map(|event_id| {
                        let (event_type, date) = events.get(&event_clusters.root(event_id))?;
                        (*event_type == Some(EventType::Marriage)).then(|| date.clone())
                    })
                    .flatten()
            });
        }
        let partners_row = partner_ids
            .into_iter()
            .map(|partner_id| {
                partners.get(&partner_id).cloned().unwrap_or_else(|| FamilyPartnerRow {
                    human_id: partner_id.to_string(),
                    name: None,
                })
            })
            .collect();
        rows.push(FamilyRow {
            human_id: root.human_id().map(ToString::to_string).unwrap_or_default(),
            partners: partners_row,
            marriage_date,
            child_count,
        });
    }
    Ok(rows)
}

/// Lists the families a person belongs to, with their role in each (partner or child).
///
/// Scans every family for one referencing the person — or any record of its cluster (ADR 0039 §5) —
/// as a partner or a child, resolving member `PersonId`s back to their roots' `human_id`s via a single
/// lookup. Returns the families in `human_id` order.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, or a store/read-model error.
pub async fn families_for_person(
    workspace: &Workspace,
    person_human_id: &str,
) -> Result<Vec<FamilyForPerson>, AppError> {
    let store = workspace.store();
    let clusters = PersonClusters::load(store).await?;
    let person_id = clusters.root(resolve_person_id(store, person_human_id).await?);
    let persons: HashMap<PersonId, String> = store
        .list_persons()
        .await?
        .iter()
        .filter_map(|p| Some((p.person_id()?, p.human_id()?.to_string())))
        .collect();
    let resolve = |id: PersonId| {
        let id = clusters.root(id);
        persons.get(&id).cloned().unwrap_or_else(|| id.to_string())
    };

    // Maps a child's per-`PersonId` relationships to per-partner-`human_id` relationships.
    let resolve_relationships = |relationships: &[(PersonId, ChildParentRelationship)]| {
        relationships
            .iter()
            .map(|(partner_id, relationship)| (resolve(*partner_id), relationship.clone()))
            .collect::<Vec<_>>()
    };

    let views = store.list_families().await?;
    let family_clusters = FamilyClusters::load(store).await?;
    let mut families = Vec::new();
    for cluster in root_clusters(&views, &family_clusters) {
        let Some(root) = cluster.first() else { continue };
        let partner_ids = cluster_partners(&cluster, &clusters);
        let children = cluster_children(&cluster, &clusters);
        let partner = partner_ids.contains(&person_id);
        let child_relationships = children
            .iter()
            .find(|child| child.child_id == person_id)
            .map(|child| resolve_relationships(&child.relationships));
        let role = match (partner, child_relationships) {
            (true, _) => PersonFamilyRole::Partner,
            (false, Some(relationships)) => PersonFamilyRole::Child(relationships),
            (false, None) => continue,
        };
        families.push(FamilyForPerson {
            family_human_id: root.human_id().map(ToString::to_string).unwrap_or_default(),
            role,
            partners: partner_ids.into_iter().map(resolve).collect(),
            children: children
                .iter()
                .map(|child| (resolve(child.child_id), resolve_relationships(&child.relationships)))
                .collect(),
        });
    }
    Ok(families)
}

/// The distinct partners of a family cluster, each resolved to its person root, in assertion order.
fn cluster_partners(cluster: &[&FamilyView], clusters: &PersonClusters) -> Vec<PersonId> {
    let mut partners = Vec::new();
    for view in cluster {
        for partner in view.partners() {
            let partner = clusters.root(partner);
            if !partners.contains(&partner) {
                partners.push(partner);
            }
        }
    }
    partners
}

/// The distinct children of a family cluster, each resolved to its person root, with the distinct
/// relationships (to partner roots) of every record that names it.
fn cluster_children(cluster: &[&FamilyView], clusters: &PersonClusters) -> Vec<ChildEntry> {
    let mut children: Vec<ChildEntry> = Vec::new();
    for view in cluster {
        for child in view.children() {
            let child_id = clusters.root(child.child_id);
            let relationships = child
                .relationships
                .into_iter()
                .map(|(partner, relationship)| (clusters.root(partner), relationship));
            match children.iter_mut().find(|held| held.child_id == child_id) {
                Some(held) => {
                    for relationship in relationships {
                        if !held.relationships.contains(&relationship) {
                            held.relationships.push(relationship);
                        }
                    }
                }
                None => children.push(ChildEntry {
                    child_id,
                    relationships: relationships.collect(),
                }),
            }
        }
    }
    children
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
pub(crate) async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: FamilyCommand,
    provenance: Provenance,
    citations: Vec<EvidenceRef>,
) -> Result<(), AppError> {
    let envelope = FamilyCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    let Some(envelope) = crate::origin_gate::gate(store, session, aggregate_id, envelope).await? else {
        return Ok(());
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
    let citations = use_case::resolve_evidence_refs(store, meta.citations, meta.dna_matches).await?;
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
        vitni_core::citation::CitationView::citation_id,
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
        vitni_core::media::MediaView::media_id,
        || AppError::MediaNotFound(human_id.to_owned()),
    )
}

/// Resolves a note `human_id` to its aggregate [`NoteId`], or [`AppError::NoteNotFound`].
async fn resolve_note_id(store: &Store, human_id: &str) -> Result<NoteId, AppError> {
    use_case::resolve_id(
        store.find_note(human_id).await?,
        vitni_core::note::NoteView::note_id,
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
    clusters: PersonClusters,
    event_clusters: EventClusters,
    events: HashMap<EventId, EventInfo>,
    citations: HashMap<CitationId, CitationRef>,
    media: HashMap<MediaId, MediaLookup>,
    notes: HashMap<NoteId, use_case::NoteLookup>,
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
                        birth_year: summary.birth_year(),
                        death_year: summary.death_year(),
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

        let mut citations = crate::dto::citation_refs(store).await?;
        let backs = crate::citation_usage::citation_backs_counts(store).await?;
        for (id, citation) in &mut citations {
            citation.backs_count = backs.get(id).copied().unwrap_or(0);
        }

        Ok(Self {
            persons,
            clusters: PersonClusters::load(store).await?,
            event_clusters: EventClusters::load(store).await?,
            events,
            citations,
            media: crate::dto::media_lookups(store).await?,
            notes: use_case::note_lookups(store).await?,
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
        .citations_with_assertions()
        .iter()
        .filter_map(|attributed| {
            lookups.citations.get(&attributed.value).cloned().map(|mut citation| {
                citation.assertion_id = Some(attributed.assertion_id.to_string());
                citation
            })
        })
        .collect();
    let media = view
        .media_with_assertions()
        .iter()
        .filter_map(|attributed| {
            let media = &attributed.value;
            lookups.media.get(&media.media_id).map(|lookup| MediaRefSummary {
                human_id: lookup.human_id.clone(),
                id: lookup.id.clone(),
                caption: media.caption.clone(),
                crop: media.crop,
                path: lookup.path.clone(),
                mime: lookup.mime.clone(),
                assertion_id: attributed.assertion_id.to_string(),
            })
        })
        .collect();
    let notes = view
        .notes_with_assertions()
        .iter()
        .filter_map(|attributed| {
            lookups.notes.get(&attributed.value).map(|note| AttachedRef {
                human_id: note.human_id.clone(),
                id: attributed.value.to_string(),
                note_type: note.note_type.clone(),
                text: note.text.clone(),
                language: note.language.clone(),
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
        merged: Vec::new(),
        claim_owners: BTreeMap::new(),
    }
}

/// Summarises a cluster — its root first, then its members — as one family (ADR 0039 §5): the root's
/// summary with every member's rows appended, each member-owned row recorded in `claim_owners`.
/// `None` for an empty slice.
fn summarize_cluster(views: &[&FamilyView], lookups: &FamilyLookups) -> Option<FamilySummary> {
    let (root, members) = views.split_first()?;
    let mut summary = summarize(root, lookups);
    for view in members {
        let member = summarize(view, lookups);
        summary.merged.push(AggRef {
            human_id: member.human_id.clone(),
            id: member.id.clone(),
        });
        adopt(&mut summary, member);
    }
    summary.merged.sort_by(|x, y| x.id.cmp(&y.id));
    Some(summary)
}

/// Appends a member's rows to its root's summary, recording the member as each row's owner.
fn adopt(root: &mut FamilySummary, member: FamilySummary) {
    let owner = member.human_id.clone();
    let mut owned: Vec<String> = Vec::new();
    owned.extend(member.partners.iter().map(|row| row.assertion_id.clone()));
    for child in &member.children {
        owned.push(child.assertion_id.clone());
        owned.extend(child.relationships.iter().map(|link| link.assertion_id.clone()));
    }
    owned.extend(member.events.iter().map(|row| row.assertion_id.clone()));
    owned.extend(member.citations.iter().filter_map(|row| row.assertion_id.clone()));
    owned.extend(member.media.iter().map(|row| row.assertion_id.clone()));
    owned.extend(member.notes.iter().map(|row| row.assertion_id.clone()));
    for assertion_id in owned {
        root.claim_owners.insert(assertion_id, owner.clone());
    }
    root.partners.extend(member.partners);
    root.children.extend(member.children);
    root.events.extend(member.events);
    root.citations.extend(member.citations);
    root.media.extend(member.media);
    root.notes.extend(member.notes);
    for tag in member.tags {
        if !root.tags.iter().any(|held| held.id == tag.id) {
            root.tags.push(tag);
        }
    }
    root.restrictions.extend(member.restrictions);
}

/// Joins each family partner to the person projection (name, lifespan, stable id) with its surety.
fn summarize_partners(view: &FamilyView, lookups: &FamilyLookups) -> Vec<PartnerRef> {
    view.partners_with_assertions()
        .iter()
        .map(|attributed| {
            let partner = &attributed.value;
            let partner_id = lookups.clusters.root(partner.value);
            let info = lookups.persons.get(&partner_id);
            PartnerRef {
                human_id: info.map_or_else(|| partner_id.to_string(), |i| i.human_id.clone()),
                id: partner_id.to_string(),
                name: info.and_then(|i| i.name.clone()),
                vitals: info.and_then(|i| crate::dto::lifespan(i.birth_year, i.death_year)),
                confidence: partner.confidence,
                source_count: partner.citation_ids().count(),
                citations: partner
                    .citations
                    .iter()
                    .filter_map(|e| e.as_citation())
                    .filter_map(|id| lookups.citations.get(&id).cloned())
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
        let partner_id = lookups.clusters.root(partner_id);
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
            let child_id = lookups.clusters.root(child.value);
            let info = lookups.persons.get(&child_id);
            let relationships = links
                .iter()
                .filter(|link| link.value.value.child_id == child.value)
                .map(|link| ChildRelationshipRef {
                    partner_human_id: resolve_partner_human(link.value.value.parent_id),
                    relationship: link.value.value.relationship.clone(),
                    confidence: link.value.confidence,
                    source_count: link.value.citation_ids().count(),
                    assertion_id: link.assertion_id.to_string(),
                })
                .collect();
            ChildRef {
                human_id: info.map_or_else(|| child_id.to_string(), |i| i.human_id.clone()),
                id: child_id.to_string(),
                name: info.and_then(|i| i.name.clone()),
                born: info.and_then(|i| i.birth_year).map(|year| year.to_string()),
                relationships,
                confidence: child.confidence,
                source_count: child.citation_ids().count(),
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
            let event_id = lookups.event_clusters.root(linked.value);
            let info = lookups.events.get(&event_id);
            FamilyEventRef {
                human_id: info.map_or_else(|| event_id.to_string(), |i| i.human_id.clone()),
                id: event_id.to_string(),
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
