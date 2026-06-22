//! Person use-cases (ADR 0006): create, name, show, and list — the operations a frontend calls.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`](genealogy_db::Store),
//! and returns a frontend-neutral [`PersonSummary`] (never a `PersonView`, cqrs-es, or sqlx type).
//! `human_id` is auto-allocated using the workspace's configured format, or validated when the
//! caller supplies one (ADR 0005).

use genealogy_core::enums::{EvidenceLevel, ParticipantRole, Sex};
use genealogy_core::event::EventView;
use genealogy_core::ids::{EventId, HumanId, PersonId};
use genealogy_core::name::{NameType, PersonName, Surname};
use genealogy_core::person::PersonView;
use genealogy_core::person::command::{PersonCommand, PersonCommandEnvelope};
use genealogy_core::provenance::CitationRef;
use genealogy_core::text::ExternalId;
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// A frontend-neutral summary of a person (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonSummary {
    /// The user-facing identifier (e.g. `I0001`).
    pub human_id: String,
    /// A display rendering of the primary name, if any name is asserted.
    pub display_name: Option<String>,
    /// The primary name's given name, if asserted — the structured part an exporter reconstructs
    /// a name from (kept distinct from `display_name`, which is for rendering).
    pub given: Option<String>,
    /// The primary name's primary surname, if asserted.
    pub surname: Option<String>,
    /// The recorded sex, if asserted. Structured (not a label) so the frontend localizes it
    /// (ADR 0003 §3 — the application layer stays string-free).
    pub sex: Option<Sex>,
    /// Whether the person is marked private.
    pub private: bool,
}

/// What to create a person with (the auto/override `human_id` and an optional initial name).
#[derive(Debug, Clone)]
pub struct NewPerson {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// An optional given name for an initial `AssertName`.
    pub given: Option<String>,
    /// An optional surname for an initial `AssertName`.
    pub surname: Option<String>,
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

    if new.given.is_some() || new.surname.is_some() {
        let name = build_name(new.given, new.surname);
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
    given: Option<String>,
    surname: Option<String>,
    provenance: Provenance,
    citations: &[String],
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let citation_refs = resolve_citation_refs(store, citations).await?;
    let name = build_name(given, surname);
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

/// Loads a single person's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_person(workspace: &Workspace, human_id: &str) -> Result<Option<PersonSummary>, AppError> {
    let found = workspace.store().find_person(human_id).await?;
    Ok(found.as_ref().map(summarize))
}

/// Lists every person's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_persons(workspace: &Workspace) -> Result<Vec<PersonSummary>, AppError> {
    let views = workspace.store().list_persons().await?;
    let mut summaries = Vec::with_capacity(views.len());
    for view in &views {
        summaries.push(summarize(view));
    }
    Ok(summaries)
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

/// Builds a [`PersonName`] from optional parts; an all-empty name is rejected downstream as
/// [`PersonError::EmptyName`](genealogy_core::person::PersonError).
pub(crate) fn build_name(given: Option<String>, surname: Option<String>) -> PersonName {
    let surnames = match surname {
        Some(surname) => vec![Surname {
            prefix: None,
            surname,
            primary: true,
            connector: None,
        }],
        None => Vec::new(),
    };
    PersonName {
        name_type: NameType::BirthName,
        given,
        surnames,
        suffix: None,
        title: None,
        nickname: None,
        call_name: None,
        date: None,
        language: None,
        transliterations: Vec::new(),
    }
}

/// Renders a [`PersonView`] into the frontend DTO.
fn summarize(view: &PersonView) -> PersonSummary {
    let human_id = view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default();
    let names = view.names();
    let primary = names.first();
    let display_name = primary.map(|name| render_name(name));
    let given = primary.and_then(|name| name.given.clone());
    let surname = primary.and_then(|name| name.surnames.first().map(|element| element.surname.clone()));
    let sex = view.sex().cloned();
    PersonSummary {
        human_id,
        display_name,
        given,
        surname,
        sex,
        private: view.is_private(),
    }
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
