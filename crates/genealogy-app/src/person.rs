//! Person use-cases (ADR 0006): create, name, show, and list — the operations a frontend calls.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`](genealogy_db::Store),
//! and returns a frontend-neutral [`PersonSummary`] (never a `PersonView`, cqrs-es, or sqlx type).
//! `human_id` is auto-allocated using the workspace's configured format, or validated when the
//! caller supplies one (ADR 0005).

use std::collections::{BTreeSet, HashMap};

use genealogy_core::date::GenealogicalDate;
use genealogy_core::enums::{AssociationRole, EvidenceLevel, FactType, ParticipantRole, Restriction, Sex};
use genealogy_core::event::EventView;
use genealogy_core::fact::Fact;
use genealogy_core::ids::{CitationId, EventId, HumanId, MediaId, NoteId, PersonId, TagId};
use genealogy_core::name::{NameType, PersonName, Surname};
use genealogy_core::person::PersonView;
use genealogy_core::person::command::{PersonCommand, PersonCommandEnvelope};
use genealogy_core::provenance::{CitationRef, Confidence};
use genealogy_core::text::{ExternalId, MediaRef};
use genealogy_db::Store;

use crate::dto::{AggRef, MediaRefSummary};
use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, Provenance};
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
    /// Notes attached to the person (e.g. `INDI.NOTE`), with stable ids, in assertion order.
    pub notes: Vec<AggRef>,
    /// Ids of tags applied to the person, in assertion order.
    pub tags: Vec<String>,
    /// The person's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// An asserted fact together with the confidence the asserting operator stamped on it
/// (data-model §7–§8). The fact's own `citations` give the source count; `confidence` is the
/// surety denormalized from the assertion's provenance envelope.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FactSummary {
    /// The asserted fact (type, date, place, value, per-fact citations).
    pub fact: Fact,
    /// The operator's surety when asserting it.
    pub confidence: Confidence,
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
}

/// A person's participation in an event (data-model §6, §10): the event (stable id + `human_id`), the
/// person's role, and the event's date joined from the Event projection (for the Events tab).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ParticipationRef {
    /// The event (its `human_id` + stable id), for display and navigation.
    pub event: AggRef,
    /// The person's role in the shared event.
    pub role: ParticipantRole,
    /// The event's date, if known. Structured so the frontend localizes it (ADR 0003).
    pub date: Option<GenealogicalDate>,
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
pub async fn create_person(workspace: &Workspace, session: &Session, new: NewPerson) -> Result<String, AppError> {
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

    let person_id = session.new_person_id();
    let aggregate_id = person_id.to_string();

    execute(
        store,
        session,
        &aggregate_id,
        PersonCommand::CreatePerson {
            person_id,
            human_id: HumanId::new(&human_id),
            evidence_level: new.evidence_level,
        },
        Provenance::default(),
        Vec::new(),
    )
    .await?;

    if let Some(parts) = new.name.filter(|parts| !parts.is_empty()) {
        let name = build_name(parts);
        execute(
            store,
            session,
            &aggregate_id,
            PersonCommand::AssertName { person_id, name },
            Provenance::default(),
            Vec::new(),
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
    provenance: Provenance,
    citations: &[String],
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let citation_refs = resolve_citation_refs(store, citations).await?;
    let name = build_name(name);
    execute(
        store,
        session,
        &person_id.to_string(),
        PersonCommand::AssertName { person_id, name },
        provenance,
        citation_refs,
    )
    .await
}

/// Asserts a person's sex (data-model §10).
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, or a workspace/store error.
pub async fn assert_sex(workspace: &Workspace, session: &Session, human_id: &str, sex: Sex) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    execute(
        store,
        session,
        &person_id.to_string(),
        PersonCommand::AssertSex { person_id, sex },
        Provenance::default(),
        Vec::new(),
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
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    execute(
        store,
        session,
        &person_id.to_string(),
        PersonCommand::SetRestrictions {
            person_id,
            restrictions,
        },
        Provenance::default(),
        Vec::new(),
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
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    execute(
        store,
        session,
        &person_id.to_string(),
        PersonCommand::AddExternalId { person_id, external_id },
        Provenance::default(),
        Vec::new(),
    )
    .await
}

/// Asserts that a person participated in an event, with a role (data-model §10).
///
/// `ParticipationAsserted` lives on the Person aggregate and references the event by id — the
/// self-contained cross-aggregate link of ADR 0002. The event must exist (resolved here); the role
/// is the participant's part in the shared event.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] / [`AppError::EventNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn assert_participation(
    workspace: &Workspace,
    session: &Session,
    person_human_id: &str,
    event_human_id: &str,
    role: ParticipantRole,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, person_human_id).await?;
    let event_id = resolve_event_id(store, event_human_id).await?;
    execute(
        store,
        session,
        &person_id.to_string(),
        PersonCommand::AssertParticipation {
            person_id,
            event_id,
            role,
        },
        Provenance::default(),
        Vec::new(),
    )
    .await
}

/// Asserts a single-person fact (data-model §10) — an occupation, religion, residence, and the
/// like — backed by zero or more citations and stamped with the operator's `provenance`.
///
/// `place_id` is left unset (an importer maps GEDCOM INDI attributes here). `citations` are citation
/// `human_id`s recorded in the assertion's `EventContext.citations` (the evidence chain) and on the
/// fact itself, so a frontend can show the source count; `provenance` carries the confidence/surety.
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
    provenance: Provenance,
    citations: &[String],
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let citation_refs = resolve_citation_refs(store, citations).await?;
    let fact = Fact {
        fact_type: new.fact_type,
        date: new.date,
        place_id: None,
        value: new.value,
        citations: citation_refs.clone(),
    };
    execute(
        store,
        session,
        &person_id.to_string(),
        PersonCommand::AssertFact { person_id, fact },
        provenance,
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
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, person_human_id).await?;
    let other = resolve_person_id(store, other_human_id).await?;
    execute(
        store,
        session,
        &person_id.to_string(),
        PersonCommand::AssertAssociation { person_id, other, role },
        Provenance::default(),
        Vec::new(),
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
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let citation_id = resolve_citation_id(store, citation_human_id).await?;
    execute(
        store,
        session,
        &person_id.to_string(),
        PersonCommand::AddCitation { person_id, citation_id },
        Provenance::default(),
        Vec::new(),
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
    execute(
        store,
        session,
        &person_id.to_string(),
        PersonCommand::AttachMedia { person_id, media },
        Provenance::default(),
        Vec::new(),
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
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let note_id = resolve_note_id(store, note_human_id).await?;
    execute(
        store,
        session,
        &person_id.to_string(),
        PersonCommand::AttachNote { person_id, note_id },
        Provenance::default(),
        Vec::new(),
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
    tag_id: TagId,
    remove: bool,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let command = if remove {
        PersonCommand::Untag { person_id, tag_id }
    } else {
        PersonCommand::Tag { person_id, tag_id }
    };
    execute(
        store,
        session,
        &person_id.to_string(),
        command,
        Provenance::default(),
        Vec::new(),
    )
    .await
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
struct Lookups {
    persons: HashMap<PersonId, String>,
    events: HashMap<EventId, (String, Option<GenealogicalDate>)>,
    citations: HashMap<CitationId, crate::dto::CitationRef>,
    media: HashMap<MediaId, (String, String)>,
    notes: HashMap<NoteId, String>,
}

impl Lookups {
    async fn load(store: &Store) -> Result<Self, AppError> {
        Ok(Self {
            persons: person_human_ids(store).await?,
            events: event_lookups(store).await?,
            citations: crate::dto::citation_refs(store).await?,
            media: crate::dto::media_refs(store).await?,
            notes: use_case::note_human_ids(store).await?,
        })
    }
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

/// Loads an `EventId -> (human_id, date)` lookup from the Event projection, so a person's
/// participations resolve to the event's stable id + `human_id` + date without a per-row query.
async fn event_lookups(store: &Store) -> Result<HashMap<EventId, (String, Option<GenealogicalDate>)>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_events().await? {
        if let (Some(id), Some(human_id)) = (view.event_id(), view.human_id()) {
            map.insert(id, (human_id.as_str().to_owned(), view.date().cloned()));
        }
    }
    Ok(map)
}

/// Executes one command through the store, stamping it with `provenance` (the operator's surety and
/// rationale) and `citations` (`EventContext.citations` — data-model §8), and maps the outcome to
/// [`AppError`].
async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: PersonCommand,
    provenance: Provenance,
    citations: Vec<CitationRef>,
) -> Result<(), AppError> {
    let envelope = PersonCommandEnvelope {
        meta: session.new_meta(provenance.confidence, provenance.rationale, citations),
        command,
    };
    store
        .execute_person(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Resolves citation `human_id`s to the [`CitationRef`]s that back an assertion, linking the
/// provenance envelope to real Citation aggregates (data-model §8).
async fn resolve_citation_refs(store: &Store, human_ids: &[String]) -> Result<Vec<CitationRef>, AppError> {
    let mut refs = Vec::with_capacity(human_ids.len());
    for human_id in human_ids {
        let view = store
            .find_citation(human_id)
            .await?
            .ok_or_else(|| AppError::CitationNotFound(human_id.clone()))?;
        let citation_id = view
            .citation_id()
            .ok_or_else(|| AppError::CitationNotFound(human_id.clone()))?;
        refs.push(CitationRef { citation_id });
    }
    Ok(refs)
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
    let events = &lookups.events;
    let human_id = view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default();
    let names = view.names();
    let primary = names.first();
    let display_name = primary.map(|name| render_name(name));
    let given = primary.and_then(|name| name.given.clone());
    let primary_surname = primary.and_then(|name| name.surnames.first());
    let surname = primary_surname.map(|element| element.surname.clone());
    let surname_prefix = primary_surname.and_then(|element| element.prefix.clone());
    let nickname = primary.and_then(|name| name.nickname.clone());
    let name_prefix = primary.and_then(|name| name.title.clone());
    let name_suffix = primary.and_then(|name| name.suffix.clone());
    let name_type = primary.map(|name| name.name_type.clone());
    let all_names = view
        .asserted_names()
        .into_iter()
        .map(|asserted| NameSummary {
            name: asserted.name.clone(),
            confidence: asserted.confidence,
            source_count: asserted.citations.len(),
        })
        .collect();
    let sex = view.sex().cloned();
    let facts = view
        .facts()
        .into_iter()
        .map(|asserted| FactSummary {
            fact: asserted.fact.clone(),
            confidence: asserted.confidence,
        })
        .collect();
    let associations = view
        .asserted_associations()
        .into_iter()
        .filter_map(|asserted| {
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
                })
        })
        .collect();
    let participations = view
        .participations()
        .into_iter()
        .filter_map(|participation| {
            events
                .get(&participation.event_id)
                .map(|(human_id, date)| ParticipationRef {
                    event: AggRef {
                        human_id: human_id.clone(),
                        id: participation.event_id.to_string(),
                    },
                    role: participation.role.clone(),
                    date: date.clone(),
                })
        })
        .collect();
    let (citations, media, notes) = person_attachments(view, lookups);
    let tags = view.tags().into_iter().map(|id| id.to_string()).collect();
    PersonSummary {
        human_id,
        evidence_level: view.evidence_level().unwrap_or(EvidenceLevel::Conclusion),
        display_name,
        given,
        surname,
        surname_prefix,
        nickname,
        name_prefix,
        name_suffix,
        name_type,
        names: all_names,
        sex,
        facts,
        associations,
        participations,
        citations,
        media,
        notes,
        tags,
        restrictions: view.restrictions().clone(),
    }
}

/// Resolves a person's attachments to their joined DTOs: citations (joined to the Citation/Source
/// projection), media (per-use caption + stable id), and notes (stable id + `human_id`).
fn person_attachments(
    view: &PersonView,
    lookups: &Lookups,
) -> (Vec<crate::dto::CitationRef>, Vec<MediaRefSummary>, Vec<AggRef>) {
    let citations = view
        .citations()
        .into_iter()
        .filter_map(|id| lookups.citations.get(&id).cloned())
        .collect();
    let media = view
        .media()
        .into_iter()
        .filter_map(|media| {
            lookups
                .media
                .get(&media.media_id)
                .map(|(human_id, id)| MediaRefSummary {
                    human_id: human_id.clone(),
                    id: id.clone(),
                    caption: media.caption.clone(),
                })
        })
        .collect();
    let notes = view
        .notes()
        .into_iter()
        .filter_map(|id| {
            lookups.notes.get(&id).map(|human_id| AggRef {
                human_id: human_id.clone(),
                id: id.to_string(),
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
