//! Person use-cases (ADR 0006): create, name, show, and list — the operations a frontend calls.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`](genealogy_db::Store),
//! and returns a frontend-neutral [`PersonSummary`] (never a `PersonView`, cqrs-es, or sqlx type).
//! `human_id` is auto-allocated using the workspace's configured format, or validated when the
//! caller supplies one (ADR 0005).

use std::collections::{BTreeSet, HashMap};

use genealogy_core::age::Age;
use genealogy_core::date::GenealogicalDate;
use genealogy_core::enums::{AssociationRole, EvidenceLevel, FactType, ParticipantRole, Restriction, Sex};
use genealogy_core::event::EventView;
use genealogy_core::fact::Fact;
use genealogy_core::ids::{AssertionId, CitationId, EventId, HumanId, MediaId, NoteId, PersonId, TagId};
use genealogy_core::name::{NameType, PersonName, Surname};
use genealogy_core::person::PersonView;
use genealogy_core::person::command::{PersonCommand, PersonCommandEnvelope};
use genealogy_core::provenance::{CitationRef, Confidence};
use genealogy_core::text::{Attribute, ExternalId, MediaRef};
use genealogy_db::Store;
use uuid::Uuid;

use crate::dto::{AggRef, AttachedRef, MediaRefSummary};
use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, MutationMeta, Provenance};
use crate::workspace::Workspace;

/// A frontend-neutral summary of a person (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonSummary {
    /// The user-facing identifier (e.g. `I0001`).
    pub human_id: String,
    /// Whether this is a persona (extracted from one source) or a conclusion (synthesized) —
    /// surfaced as the "personas" badge on the detail header (data-model §7).
    pub evidence_level: EvidenceLevel,
    /// A display rendering of the primary name, if any name is asserted.
    pub display_name: Option<String>,
    /// The primary name's given name, if asserted — the structured part an exporter reconstructs
    /// a name from (kept distinct from `display_name`, which is for rendering).
    pub given: Option<String>,
    /// The primary name's primary surname, if asserted.
    pub surname: Option<String>,
    /// The primary surname's prefix (GEDCOM `SPFX`, e.g. `van`), if any.
    pub surname_prefix: Option<String>,
    /// The primary name's nickname (GEDCOM `NICK`), if any.
    pub nickname: Option<String>,
    /// The primary name's title / prefix (GEDCOM `NPFX`, e.g. `Dr`), if any.
    pub name_prefix: Option<String>,
    /// The primary name's suffix (GEDCOM `NSFX`, e.g. `Jr`), if any.
    pub name_suffix: Option<String>,
    /// The primary name's type (GEDCOM `NAME.TYPE`).
    pub name_type: Option<NameType>,
    /// The `AssertionId` (a UUID string) of the primary name assertion, if any — the target an edit
    /// supersedes so the changed preferred name replaces the old one rather than becoming a second
    /// name. Never rendered; used only to build the correction command (data-model §10.1).
    pub primary_name_assertion: Option<String>,
    /// Every currently-live asserted name, in assertion order (the primary is the first), each with
    /// the surety + source count the asserting operator stamped on it. The flattened
    /// `given`/`surname`/… fields above describe the primary; this carries the rest for a names view.
    pub names: Vec<NameSummary>,
    /// The recorded sex, if asserted. Structured (not a label) so the frontend localizes it
    /// (ADR 0003 §3 — the application layer stays string-free).
    pub sex: Option<Sex>,
    /// All currently-live asserted facts (INDI attributes — data-model §7), each with the
    /// confidence the asserting operator stamped on it.
    pub facts: Vec<FactSummary>,
    /// Person-to-person associations (data-model §10), each with the other person's `human_id`, the
    /// role, and the surety + source count. The `PersonId` is resolved so a frontend needs no lookup.
    pub associations: Vec<AssociationSummary>,
    /// Event participations (data-model §6, §10), each joined to the event projection so a frontend
    /// has the event's stable id + `human_id` + date and needs no second lookup.
    pub participations: Vec<ParticipationRef>,
    /// Citations backing the person's claims (e.g. `INDI.SOUR`), joined to the Citation/Source
    /// projection (source · page · surety · evidence axes · stable ids), in assertion order.
    pub citations: Vec<crate::dto::CitationRef>,
    /// Media attached to the person (e.g. `INDI.OBJE`), with stable ids + per-use captions.
    pub media: Vec<MediaRefSummary>,
    /// Notes attached to the person (e.g. `INDI.NOTE`), with stable ids + the attach `AssertionId`
    /// (the Detach target), in assertion order.
    pub notes: Vec<AttachedRef>,
    /// Ids of tags applied to the person, in assertion order (the edit change-set diffs against
    /// these; the display renders [`tag_refs`](Self::tag_refs) instead — never the id).
    pub tags: Vec<String>,
    /// The applied tags resolved to name + colour + priority (never rendered by id — data-model §9),
    /// in assertion order. Built by joining each applied tag id to the Tag projection.
    pub tag_refs: Vec<crate::citation::TagRef>,
    /// The person's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
    /// Personas merged into this person (data-model §9) — the survivor side of a `PersonsMerged`
    /// event whose assertion has not been undone.
    pub merged: Vec<AggRef>,
}

/// An asserted fact together with the surety + citations denormalized from its provenance envelope
/// (data-model §7–§8). The envelope citations give the source count; `confidence` is the surety.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactSummary {
    /// The asserted fact (type, date, place, value).
    pub fact: Fact,
    /// The operator's surety when asserting it.
    pub confidence: Confidence,
    /// The fact's citations (from `EventContext.citations`, the sole evidence channel — ADR 0020),
    /// joined to the source projection (title, page, surety, evidence axes) — the evidence behind
    /// this fact, for the provenance popover.
    pub citations: Vec<crate::dto::CitationRef>,
    /// The `AssertionId` (a UUID string) that introduced this fact — the target a per-row Edit
    /// supersedes and a Retract retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// An asserted name with the surety + source count denormalized from its provenance envelope
/// (data-model §7–§8), so a names view can show evidence cues per row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NameSummary {
    /// The asserted name.
    pub name: PersonName,
    /// The operator's surety when asserting it.
    pub confidence: Confidence,
    /// How many citations back the name (its source count).
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) that introduced this name — the target a per-row Edit
    /// supersedes and a Retract retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// An asserted person-to-person association with the other person (stable id + `human_id`), the role,
/// and the surety + source count (data-model §8, §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssociationSummary {
    /// The associated person (their `human_id` + stable id), for display and navigation.
    pub other: AggRef,
    /// The kind of association.
    pub role: AssociationRole,
    /// The operator's surety when asserting it.
    pub confidence: Confidence,
    /// How many citations back the association (its source count).
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) that introduced this association — the target a per-row
    /// Edit supersedes and a Retract retracts (ADR 0004 §2). Never rendered.
    pub assertion_id: String,
}

/// A person's participation in an event (data-model §6, §10): the event (stable id + `human_id`), the
/// person's role, the event's date joined from the Event projection (for the Events tab), and the
/// participant-scoped detail a person-side assertion carries — the age at the event, typed
/// attributes, and resolved notes (ADR 0019), plus the surety + source count denormalized from the
/// assertion envelope (ADR 0020).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipationRef {
    /// The event (its `human_id` + stable id), for display and navigation.
    pub event: AggRef,
    /// The person's role in the shared event.
    pub role: ParticipantRole,
    /// The event's date, if known. Structured so the frontend localizes it (ADR 0003).
    pub date: Option<GenealogicalDate>,
    /// The participant's age at the event, if recorded (ADR 0019).
    pub age: Option<Age>,
    /// Participant-scoped typed attributes (ADR 0019).
    pub attributes: Vec<Attribute>,
    /// Notes about this participation, resolved to their `human_id` + stable id (ADR 0019).
    pub notes: Vec<AggRef>,
    /// The operator's surety when asserting the participation (denormalized from the envelope).
    pub confidence: Confidence,
    /// How many citations back the participation (its source count, from the envelope — ADR 0020).
    pub source_count: usize,
    /// The `AssertionId` (a UUID string) of the person-side `ParticipationAsserted` that introduced
    /// this participation — the target a per-row Edit (change role) supersedes and a Retract retracts
    /// (ADR 0004 §2). Never rendered. Always the Person-aggregate assertion (the single owner).
    pub assertion_id: String,
}

/// What to assert a fact with (the fact's type and its optional value and date). `place_id` and
/// citations are supplied separately by the use-case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewFact {
    /// The fact's type (GEDCOM INDI attribute — data-model §7).
    pub fact_type: FactType,
    /// The fact's free-text value (e.g. an occupation title), if any.
    pub value: Option<String>,
    /// The fact's date, if any.
    pub date: Option<GenealogicalDate>,
}

/// What to assert a participation with (data-model §6, §10; ADR 0019): the participant's role, and the
/// participant-scoped detail a source records — the age at the event, typed attributes, and notes (by
/// their `human_id`, resolved by the use-case). Backing citations and surety travel on the
/// [`MutationMeta`], the sole evidence channel (ADR 0020).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewParticipation {
    /// The participant's role in the shared event.
    pub role: ParticipantRole,
    /// The participant's age at the event, if recorded.
    pub age: Option<Age>,
    /// Participant-scoped typed attributes (e.g. a witness's recorded occupation).
    pub attributes: Vec<Attribute>,
    /// The `human_id`s of notes about this participation (resolved to `NoteId`s by the use-case).
    pub notes: Vec<String>,
}

impl NewParticipation {
    /// A participation with only a role — the common event-screen case (no age/attributes/notes).
    #[must_use]
    pub fn with_role(role: ParticipantRole) -> Self {
        Self {
            role,
            age: None,
            attributes: Vec::new(),
            notes: Vec::new(),
        }
    }
}

/// The structured parts of a person's name an importer parses and an exporter reconstructs
/// (data-model §7 / GEDCOM 7 `NAME` sub-records). `simple` covers the given+surname-only case.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonNameParts {
    /// The name type (GEDCOM `NAME.TYPE`).
    pub name_type: NameType,
    /// The given name (GEDCOM `GIVN`).
    pub given: Option<String>,
    /// The primary surname's prefix (GEDCOM `SPFX`).
    pub surname_prefix: Option<String>,
    /// The primary surname (GEDCOM `SURN`).
    pub surname: Option<String>,
    /// The nickname (GEDCOM `NICK`).
    pub nickname: Option<String>,
    /// The name prefix / title (GEDCOM `NPFX`) — mapped to `PersonName.title`.
    pub prefix: Option<String>,
    /// The name suffix (GEDCOM `NSFX`).
    pub suffix: Option<String>,
}

impl PersonNameParts {
    /// A name with only a given name and a primary surname (the common CLI case).
    #[must_use]
    pub fn simple(given: Option<String>, surname: Option<String>) -> Self {
        Self {
            name_type: NameType::BirthName,
            given,
            surname_prefix: None,
            surname,
            nickname: None,
            prefix: None,
            suffix: None,
        }
    }

    /// Whether every part is absent (so no name should be asserted).
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.given.is_none()
            && self.surname.is_none()
            && self.surname_prefix.is_none()
            && self.nickname.is_none()
            && self.prefix.is_none()
            && self.suffix.is_none()
    }
}

/// What to create a person with (the auto/override `human_id` and an optional initial name).
#[derive(Debug, Clone)]
pub struct NewPerson {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// An optional initial name to `AssertName`.
    pub name: Option<PersonNameParts>,
    /// Whether this is a persona or a conclusion.
    pub evidence_level: EvidenceLevel,
}

/// Creates a person, returning the assigned `human_id`.
///
/// Resolves the `human_id` (auto-allocated via the workspace format, or validated-unique if
/// supplied), then emits `CreatePerson` and — if a name was given — `AssertName`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::Domain`] if a domain rule
/// rejects the command (e.g. an empty name), or a workspace/store error.
pub async fn create_person(
    workspace: &Workspace,
    session: &Session,
    new: NewPerson,
    provenance: Provenance,
    citations: &[String],
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_person(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_person_human_id(&workspace.person_id_format()?).await?,
    };
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;

    let person_id = session.new_person_id();
    let aggregate_id = person_id.to_string();

    execute_person_command(
        store,
        session,
        &aggregate_id,
        PersonCommand::CreatePerson {
            person_id,
            human_id: HumanId::new(&human_id),
            evidence_level: new.evidence_level,
        },
        provenance.clone(),
        Vec::new(),
    )
    .await?;

    if let Some(parts) = new.name.filter(|parts| !parts.is_empty()) {
        let name = build_name(parts);
        execute_person_command(
            store,
            session,
            &aggregate_id,
            PersonCommand::AssertName { person_id, name },
            provenance,
            citation_refs,
        )
        .await?;
    }

    Ok(human_id)
}

/// Asserts an additional name on an existing person, backed by zero or more citations.
///
/// `citations` are citation `human_id`s; each is resolved to a [`CitationRef`] and recorded in the
/// assertion's `EventContext.citations`, linking the claim to real Citation aggregates
/// (data-model §8) — the evidence chain Source ← Citation ← assertion.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, [`AppError::CitationNotFound`] if a cited
/// citation is unknown, [`AppError::Domain`] if the name is empty, or a workspace/store error.
pub async fn add_name(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    name: PersonNameParts,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let name = build_name(name);
    execute_person_mutation(
        store,
        session,
        person_id,
        PersonCommand::AssertName { person_id, name },
        meta,
    )
    .await
}

/// Asserts a person's sex (data-model §10).
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, or a workspace/store error.
pub async fn assert_sex(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    sex: Sex,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    execute_person_mutation(
        store,
        session,
        person_id,
        PersonCommand::AssertSex { person_id, sex },
        meta,
    )
    .await
}

/// Sets a person's privacy restrictions (GEDCOM `RESN` — data-model §6).
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    execute_person_mutation(
        store,
        session,
        person_id,
        PersonCommand::SetRestrictions {
            person_id,
            restrictions,
        },
        meta,
    )
    .await
}

/// Records a stable external identifier on a person (data-model §11).
///
/// Idempotent in the core: re-adding the same `(authority, value)` emits no event. The resolution
/// key behind re-import (see [`crate::import`]).
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, or a workspace/store error.
pub async fn add_external_id(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    external_id: ExternalId,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    execute_person_mutation(
        store,
        session,
        person_id,
        PersonCommand::AddExternalId { person_id, external_id },
        meta,
    )
    .await
}

/// Asserts that a person participated in an event, with a role and the participant-scoped detail a
/// source records — the age at the event, typed attributes, and notes (data-model §6, §10; ADR 0019).
///
/// `ParticipationAsserted` lives on the Person aggregate and references the event by id — the
/// self-contained cross-aggregate link of ADR 0002. The event must exist (resolved here); each note
/// `human_id` in `new.notes` is resolved to its `NoteId` (an unknown one is [`AppError::NoteNotFound`]).
///
/// # Errors
///
/// [`AppError::PersonNotFound`] / [`AppError::EventNotFound`] / [`AppError::NoteNotFound`] if the
/// person, event, or a cited note does not exist, or a workspace/store error.
pub async fn assert_participation(
    workspace: &Workspace,
    session: &Session,
    person_human_id: &str,
    event_human_id: &str,
    new: NewParticipation,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, person_human_id).await?;
    let event_id = resolve_event_id(store, event_human_id).await?;
    let mut notes = Vec::with_capacity(new.notes.len());
    for note_human_id in &new.notes {
        notes.push(resolve_note_id(store, note_human_id).await?);
    }
    execute_person_mutation(
        store,
        session,
        person_id,
        PersonCommand::AssertParticipation {
            person_id,
            event_id,
            role: new.role,
            age: new.age.filter(|age| !age.is_empty()),
            attributes: new.attributes,
            notes,
        },
        meta,
    )
    .await
}

/// Asserts a single-person fact (data-model §10) — an occupation, religion, residence, and the
/// like — backed by zero or more citations and stamped with the operator's `provenance`.
///
/// `place_id` is left unset (an importer maps GEDCOM INDI attributes here). `citations` are citation
/// `human_id`s recorded in the assertion's `EventContext.citations` — the sole evidence channel
/// (ADR 0020), which the projection denormalizes so a frontend can show the source count;
/// `provenance` carries the confidence/surety.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, [`AppError::CitationNotFound`] if a cited
/// citation is unknown, or a workspace/store error.
pub async fn assert_fact(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    new: NewFact,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let citation_refs = use_case::resolve_citation_refs(store, meta.citations).await?;
    let target = use_case::parse_supersedes(meta.supersedes)?;
    let fact = Fact {
        fact_type: new.fact_type,
        date: new.date,
        place_id: None,
        value: new.value,
    };
    let command = superseded(person_id, PersonCommand::AssertFact { person_id, fact }, target);
    execute_person_command(
        store,
        session,
        &person_id.to_string(),
        command,
        meta.provenance,
        citation_refs,
    )
    .await
}

/// Asserts a person-to-person association with a role (GEDCOM 7 `ASSO` — data-model §10).
///
/// Both persons are resolved by `human_id`; the association is recorded on the asserting person and
/// references the other by id (the self-contained cross-aggregate link of ADR 0002).
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if either person does not exist, [`AppError::Domain`] if the core
/// rejects it (e.g. `SelfAssociation`), or a workspace/store error.
pub async fn assert_association(
    workspace: &Workspace,
    session: &Session,
    person_human_id: &str,
    other_human_id: &str,
    role: AssociationRole,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, person_human_id).await?;
    let other = resolve_person_id(store, other_human_id).await?;
    execute_person_mutation(
        store,
        session,
        person_id,
        PersonCommand::AssertAssociation { person_id, other, role },
        meta,
    )
    .await
}

/// Adds a citation backing the person's claims (e.g. a GEDCOM `INDI.SOUR`).
///
/// # Errors
///
/// [`AppError::PersonNotFound`] / [`AppError::CitationNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn add_person_citation(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    citation_human_id: &str,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let citation_id = resolve_citation_id(store, citation_human_id).await?;
    execute_person_mutation(
        store,
        session,
        person_id,
        PersonCommand::AddCitation { person_id, citation_id },
        meta,
    )
    .await
}

/// Attaches a media object to the person (e.g. a GEDCOM `INDI.OBJE`).
///
/// # Errors
///
/// [`AppError::PersonNotFound`] / [`AppError::MediaNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn attach_person_media(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    media_human_id: &str,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let media_id = resolve_media_id(store, media_human_id).await?;
    let media = MediaRef {
        media_id,
        crop: None,
        caption: None,
        citations: Vec::new(),
    };
    execute_person_mutation(
        store,
        session,
        person_id,
        PersonCommand::AttachMedia { person_id, media },
        meta,
    )
    .await
}

/// Attaches a note to the person (e.g. a GEDCOM `INDI.NOTE`).
///
/// # Errors
///
/// [`AppError::PersonNotFound`] / [`AppError::NoteNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn attach_person_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_human_id: &str,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let note_id = resolve_note_id(store, note_human_id).await?;
    execute_person_mutation(
        store,
        session,
        person_id,
        PersonCommand::AttachNote { person_id, note_id },
        meta,
    )
    .await
}

/// Applies (or, with `remove`, removes) a tag on the person.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, or a workspace/store error.
pub async fn tag_person(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: &str,
    remove: bool,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    let command = if remove {
        PersonCommand::Untag { person_id, tag_id }
    } else {
        PersonCommand::Tag { person_id, tag_id }
    };
    execute_person_mutation(store, session, person_id, command, meta).await
}

/// Parses a tag aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`]. Mirrors
/// the other aggregates' tag-id parse (data-model §9; the UI carries the id, never renders it).
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// The outcome of [`merge_persons`]: the survivor's refreshed summary, the merged person's
/// `human_id`, and how many other records still reference the merged person's id.
///
/// `still_referenced` is deliberately *not* framed as "relationships re-pointed" — the merge is a
/// same-as/evidence link on the survivor (data-model §9); no Family/Association/Participation record
/// that names the merged person is rewritten. Those records keep working unchanged (their id still
/// resolves), they are simply not repointed at the survivor.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MergeResult {
    /// The survivor's summary after the merge (carries the new persona in `merged`).
    pub survivor: PersonSummary,
    /// The merged person's `human_id` (their own record/stream is untouched and still resolvable).
    pub merged_human_id: String,
    /// How many other records (family partner/child slots, person associations/participations) still
    /// name the merged person's id.
    pub still_referenced: usize,
}

/// Merges `merged_human_id` into `surviving_human_id`, recording a same-as link on the survivor.
///
/// Emits a single `MergePersons` event on the *surviving* person's stream (data-model §9). This is
/// non-destructive: the merged person's own event stream, and every existing Family/Association/
/// Participation record naming their id, is left exactly as it was — `merge_persons` does not
/// re-point any cross-aggregate reference (no core command exists to do that, and none is added
/// here). The merged person becomes a linked persona of the survivor; both streams are retained.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if either `human_id` does not resolve, [`AppError::Domain`] (via
/// [`PersonError::MergeConflict`](genealogy_core::person::PersonError::MergeConflict)) if the two
/// `human_id`s resolve to the same person, or a workspace/store error.
pub async fn merge_persons(
    workspace: &Workspace,
    session: &Session,
    surviving_human_id: &str,
    merged_human_id: &str,
    rationale: Option<String>,
) -> Result<MergeResult, AppError> {
    let store = workspace.store();
    let surviving = resolve_person_id(store, surviving_human_id).await?;
    let merged = resolve_person_id(store, merged_human_id).await?;
    let provenance = Provenance {
        confidence: Confidence::Normal,
        rationale: Some(rationale.unwrap_or_else(|| "Merge".to_owned())),
        evidence_analysis: None,
    };
    execute_person_command(
        store,
        session,
        &surviving.to_string(),
        PersonCommand::MergePersons { surviving, merged },
        provenance,
        Vec::new(),
    )
    .await?;

    let survivor = show_person(workspace, surviving_human_id)
        .await?
        .ok_or_else(|| AppError::PersonNotFound(surviving_human_id.to_owned()))?;
    let still_referenced = crate::merge_usage::count_references(workspace, merged).await?;
    Ok(MergeResult {
        survivor,
        merged_human_id: merged_human_id.to_owned(),
        still_referenced,
    })
}

/// Loads a single person's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_person(workspace: &Workspace, human_id: &str) -> Result<Option<PersonSummary>, AppError> {
    let store = workspace.store();
    let Some(found) = store.find_person(human_id).await? else {
        return Ok(None);
    };
    let lookups = Lookups::load(store).await?;
    Ok(Some(summarize(&found, &lookups)))
}

/// Lists every person's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_persons(workspace: &Workspace) -> Result<Vec<PersonSummary>, AppError> {
    let store = workspace.store();
    let views = store.list_persons().await?;
    let lookups = Lookups::load(store).await?;
    let mut summaries = Vec::with_capacity(views.len());
    for view in &views {
        summaries.push(summarize(view, &lookups));
    }
    Ok(summaries)
}

/// The `id -> human_id` lookups `summarize` needs to resolve a person's cross-aggregate references
/// (associations, participations) and attachments (citations, media, notes) without a per-row query.
///
/// `events` resolves each `ParticipationAsserted.event_id` to the event's stable id + `human_id` +
/// date, so the person's Events tab shows the event a participation references (data-model §6, §10).
struct Lookups {
    persons: HashMap<PersonId, String>,
    events: HashMap<EventId, (String, Option<GenealogicalDate>)>,
    citations: HashMap<CitationId, crate::dto::CitationRef>,
    media: HashMap<MediaId, (String, String)>,
    notes: HashMap<NoteId, String>,
    tags: HashMap<TagId, crate::citation::TagRef>,
}

impl Lookups {
    async fn load(store: &Store) -> Result<Self, AppError> {
        Ok(Self {
            persons: person_human_ids(store).await?,
            events: event_lookups(store).await?,
            citations: crate::dto::citation_refs(store).await?,
            media: crate::dto::media_refs(store).await?,
            notes: use_case::note_human_ids(store).await?,
            tags: tag_labels(store).await?,
        })
    }
}

/// Builds a `TagId -> TagRef` lookup from the Tag projection, so a person's applied tags render by
/// name/colour/priority (never by id — data-model §9).
async fn tag_labels(store: &Store) -> Result<HashMap<TagId, crate::citation::TagRef>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_tags().await? {
        if let (Some(id), Some(name)) = (view.tag_id(), view.name()) {
            map.insert(
                id,
                crate::citation::TagRef {
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

/// Builds a `PersonId -> human_id` lookup from already-loaded person views, to resolve association
/// targets without a second query.
fn person_id_map(views: &[PersonView]) -> HashMap<PersonId, String> {
    let mut map = HashMap::with_capacity(views.len());
    for view in views {
        if let (Some(id), Some(human_id)) = (view.person_id(), view.human_id()) {
            map.insert(id, human_id.as_str().to_owned());
        }
    }
    map
}

/// Loads the `PersonId -> human_id` lookup from the Person projection (for resolving associations).
async fn person_human_ids(store: &Store) -> Result<HashMap<PersonId, String>, AppError> {
    Ok(person_id_map(&store.list_persons().await?))
}

/// Loads the `EventId -> (human_id, date)` lookup from the Event projection so a person's
/// participations resolve to the event's stable id + `human_id` + date (without a per-row query).
async fn event_lookups(store: &Store) -> Result<HashMap<EventId, (String, Option<GenealogicalDate>)>, AppError> {
    let mut events = HashMap::new();
    for view in store.list_events().await? {
        let Some(id) = view.event_id() else {
            continue;
        };
        if let Some(human_id) = view.human_id() {
            events.insert(id, (human_id.as_str().to_owned(), view.date().cloned()));
        }
    }
    Ok(events)
}

/// Executes one command through the store, stamping it with `provenance` (the operator's surety and
/// rationale) and `citations` (`EventContext.citations` — data-model §8), and maps the outcome to
/// [`AppError`]. Shared with the change-set use-case ([`crate::person_change_set`]).
/// Sets (or changes) a person's user-facing identifier, identified by its current `human_id`,
/// returning the effective new id.
///
/// A supplied non-blank `new` id is dup-checked (a collision with a *different* record is
/// [`AppError::HumanIdTaken`]); a blank/absent `new` allocates the next free id from the workspace's
/// configured format (the regenerate case). The whole-record Person save runs this after
/// [`commit_person_change_set`](crate::person_change_set::commit_person_change_set) when the id
/// changed: the id is a last-writer-wins identity attribute, distinct from the claims the change-set
/// diffs, so it is a separate audited command.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if the person is unknown, [`AppError::HumanIdTaken`] if the requested
/// id is already in use, or a workspace/store error.
pub async fn set_person_human_id(
    workspace: &Workspace,
    session: &Session,
    current_human_id: &str,
    new: Option<String>,
    provenance: Provenance,
) -> Result<String, AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, current_human_id).await?;
    let human_id = match use_case::requested_human_id(new) {
        Some(id) => {
            if id != current_human_id && store.find_person(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_person_human_id(&workspace.person_id_format()?).await?,
    };
    execute_person_command(
        store,
        session,
        &person_id.to_string(),
        PersonCommand::SetHumanId {
            person_id,
            human_id: HumanId::new(&human_id),
        },
        provenance,
        Vec::new(),
    )
    .await?;
    Ok(human_id)
}

pub(crate) async fn execute_person_command(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: PersonCommand,
    provenance: Provenance,
    citations: Vec<CitationRef>,
) -> Result<(), AppError> {
    let envelope = PersonCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_person(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Executes one non-create person mutation, applying the operator-intent [`MutationMeta`]: resolves
/// the backing citations, and — when `meta.supersedes` is set — wraps `command` in a
/// [`PersonCommand::SupersedeAssertion`] so the new assertion replaces the named one (ADR 0004 §2).
async fn execute_person_mutation(
    store: &Store,
    session: &Session,
    person_id: PersonId,
    command: PersonCommand,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    let target = use_case::parse_supersedes(meta.supersedes)?;
    let command = superseded(person_id, command, target);
    execute_person_command(
        store,
        session,
        &person_id.to_string(),
        command,
        meta.provenance,
        citations,
    )
    .await
}

/// Wraps `command` in a [`PersonCommand::SupersedeAssertion`] against `target` when superseding, or
/// returns it unchanged for a plain assertion.
fn superseded(person_id: PersonId, command: PersonCommand, target: Option<AssertionId>) -> PersonCommand {
    match target {
        Some(target) => PersonCommand::SupersedeAssertion {
            person_id,
            target,
            replacement: Box::new(command),
        },
        None => command,
    }
}

/// Resolves a `human_id` to its aggregate [`PersonId`], or [`AppError::PersonNotFound`] — the
/// crate-internal accessor the change-set use-case ([`crate::person_change_set`]) reuses.
pub(crate) async fn resolve_person_id_public(store: &Store, human_id: &str) -> Result<PersonId, AppError> {
    resolve_person_id(store, human_id).await
}

/// Resolves a `human_id` to its aggregate [`PersonId`], or [`AppError::PersonNotFound`].
async fn resolve_person_id(store: &Store, human_id: &str) -> Result<PersonId, AppError> {
    use_case::resolve_id(store.find_person(human_id).await?, PersonView::person_id, || {
        AppError::PersonNotFound(human_id.to_owned())
    })
}

/// Resolves an event `human_id` to its aggregate [`EventId`], or [`AppError::EventNotFound`].
async fn resolve_event_id(store: &Store, human_id: &str) -> Result<EventId, AppError> {
    use_case::resolve_id(store.find_event(human_id).await?, EventView::event_id, || {
        AppError::EventNotFound(human_id.to_owned())
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

/// Builds a [`PersonName`] from structured parts; an all-empty name is rejected downstream as
/// [`PersonError::EmptyName`](genealogy_core::person::PersonError).
pub(crate) fn build_name(parts: PersonNameParts) -> PersonName {
    let surnames = match parts.surname {
        Some(surname) => vec![Surname {
            prefix: parts.surname_prefix,
            surname,
            primary: true,
            connector: None,
        }],
        None => Vec::new(),
    };
    PersonName {
        name_type: parts.name_type,
        given: parts.given,
        surnames,
        suffix: parts.suffix,
        title: parts.prefix,
        nickname: parts.nickname,
        call_name: None,
        date: None,
        language: None,
        transliterations: Vec::new(),
    }
}

/// Renders a [`PersonView`] into the frontend DTO, resolving association targets to their `human_id`
/// via `persons` and participation events via `events`.
fn summarize(view: &PersonView, lookups: &Lookups) -> PersonSummary {
    let persons = &lookups.persons;
    let human_id = view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default();
    let names = view.names();
    let primary = primary_name_fields(names.first().copied());
    let all_names = view
        .names_with_assertions()
        .iter()
        .map(|attributed| NameSummary {
            name: attributed.value.name.clone(),
            confidence: attributed.value.confidence,
            source_count: attributed.value.citations.len(),
            assertion_id: attributed.assertion_id.to_string(),
        })
        .collect();
    let sex = view.sex().cloned();
    let facts = view
        .facts_with_assertions()
        .iter()
        .map(|attributed| FactSummary {
            fact: attributed.value.fact.clone(),
            confidence: attributed.value.confidence,
            citations: attributed
                .value
                .citations
                .iter()
                .filter_map(|id| lookups.citations.get(id).cloned())
                .collect(),
            assertion_id: attributed.assertion_id.to_string(),
        })
        .collect();
    let associations = view
        .associations_with_assertions()
        .iter()
        .filter_map(|attributed| {
            let asserted = &attributed.value;
            persons
                .get(&asserted.association.other)
                .map(|human_id| AssociationSummary {
                    other: AggRef {
                        human_id: human_id.clone(),
                        id: asserted.association.other.to_string(),
                    },
                    role: asserted.association.role.clone(),
                    confidence: asserted.confidence,
                    source_count: asserted.citations.len(),
                    assertion_id: attributed.assertion_id.to_string(),
                })
        })
        .collect();
    let participations = merged_participations(view, lookups);
    let (citations, media, notes) = person_attachments(view, lookups);
    let (tags, tag_refs) = person_tags(view, lookups);
    let merged = person_merged(view, persons);
    PersonSummary {
        human_id,
        evidence_level: view.evidence_level().unwrap_or(EvidenceLevel::Conclusion),
        display_name: primary.display_name,
        given: primary.given,
        surname: primary.surname,
        surname_prefix: primary.surname_prefix,
        nickname: primary.nickname,
        name_prefix: primary.name_prefix,
        name_suffix: primary.name_suffix,
        name_type: primary.name_type,
        primary_name_assertion: view.primary_name_assertion().map(|id| id.to_string()),
        names: all_names,
        sex,
        facts,
        associations,
        participations,
        citations,
        media,
        notes,
        tags,
        tag_refs,
        restrictions: view.restrictions().clone(),
        merged,
    }
}

/// The primary name's flattened display/structured fields (data-model §7) — the first of
/// [`PersonView::names`], or all-`None` for an unnamed person.
struct PrimaryNameFields {
    display_name: Option<String>,
    given: Option<String>,
    surname: Option<String>,
    surname_prefix: Option<String>,
    nickname: Option<String>,
    name_prefix: Option<String>,
    name_suffix: Option<String>,
    name_type: Option<NameType>,
}

/// Derives the [`PrimaryNameFields`] from the person's primary (first-asserted) name, if any.
fn primary_name_fields(primary: Option<&PersonName>) -> PrimaryNameFields {
    let primary_surname = primary.and_then(|name| name.surnames.first());
    PrimaryNameFields {
        display_name: primary.map(render_name),
        given: primary.and_then(|name| name.given.clone()),
        surname: primary_surname.map(|element| element.surname.clone()),
        surname_prefix: primary_surname.and_then(|element| element.prefix.clone()),
        nickname: primary.and_then(|name| name.nickname.clone()),
        name_prefix: primary.and_then(|name| name.title.clone()),
        name_suffix: primary.and_then(|name| name.suffix.clone()),
        name_type: primary.map(|name| name.name_type.clone()),
    }
}

/// Projects a person's event participations from the canonical `ParticipationAsserted` rows on the
/// Person aggregate (the single owner — data-model §6, §10, ADR 0019), each joined to its event via
/// the Event projection. Only participations whose event resolves in the projection appear.
fn merged_participations(view: &PersonView, lookups: &Lookups) -> Vec<ParticipationRef> {
    let events = &lookups.events;
    view.participations_with_assertions()
        .iter()
        .filter_map(|attributed| {
            let asserted = &attributed.value;
            let participation = &asserted.value;
            events
                .get(&participation.event_id)
                .map(|(human_id, date)| ParticipationRef {
                    event: AggRef {
                        human_id: human_id.clone(),
                        id: participation.event_id.to_string(),
                    },
                    role: participation.role.clone(),
                    date: date.clone(),
                    age: participation.age.clone(),
                    attributes: participation.attributes.clone(),
                    notes: participation
                        .notes
                        .iter()
                        .filter_map(|note_id| {
                            lookups.notes.get(note_id).map(|human_id| AggRef {
                                human_id: human_id.clone(),
                                id: note_id.to_string(),
                            })
                        })
                        .collect(),
                    confidence: asserted.confidence,
                    source_count: asserted.citations.len(),
                    assertion_id: attributed.assertion_id.to_string(),
                })
        })
        .collect()
}

/// Resolves the personas merged into this survivor to their `human_id` + stable id (data-model §9).
fn person_merged(view: &PersonView, persons: &HashMap<PersonId, String>) -> Vec<AggRef> {
    view.merged()
        .into_iter()
        .filter_map(|id| {
            persons.get(&id).map(|human_id| AggRef {
                human_id: human_id.clone(),
                id: id.to_string(),
            })
        })
        .collect()
}

/// Resolves a person's applied tags to both the raw id list (for the edit change-set diff) and the
/// name/colour-resolved [`TagRef`](crate::citation::TagRef)s (for display — never rendered by id).
fn person_tags(view: &PersonView, lookups: &Lookups) -> (Vec<String>, Vec<crate::citation::TagRef>) {
    let applied = view.tags();
    let ids = applied.iter().map(ToString::to_string).collect();
    let refs = applied.iter().filter_map(|id| lookups.tags.get(id).cloned()).collect();
    (ids, refs)
}

/// Resolves a person's attachments to their joined DTOs: citations (joined to the Citation/Source
/// projection), media (per-use caption + stable id), and notes (stable id + `human_id`).
fn person_attachments(
    view: &PersonView,
    lookups: &Lookups,
) -> (Vec<crate::dto::CitationRef>, Vec<MediaRefSummary>, Vec<AttachedRef>) {
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
            lookups
                .media
                .get(&attributed.value.media_id)
                .map(|(human_id, id)| MediaRefSummary {
                    human_id: human_id.clone(),
                    id: id.clone(),
                    caption: attributed.value.caption.clone(),
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
    (citations, media, notes)
}

/// Renders a name as `given primary-surname(s)` for display.
fn render_name(name: &PersonName) -> String {
    let mut parts: Vec<&str> = Vec::new();
    if let Some(given) = name.given.as_deref() {
        parts.push(given);
    }
    for surname in &name.surnames {
        parts.push(&surname.surname);
    }
    parts.join(" ")
}
