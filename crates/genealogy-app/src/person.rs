//! Person use-cases (ADR 0006): create, name, show, and list — the operations a frontend calls.
//!
//! Each builds a command + [`AssertionMeta`](genealogy_core::provenance::AssertionMeta) from the
//! [`Session`], executes it through the workspace's engine-neutral [`Store`](genealogy_db::Store),
//! and returns a frontend-neutral [`PersonSummary`] (never a `PersonView`, cqrs-es, or sqlx type).
//! `human_id` is auto-allocated using the workspace's configured format, or validated when the
//! caller supplies one (ADR 0005).

use genealogy_core::enums::{EvidenceLevel, Sex};
use genealogy_core::ids::{HumanId, PersonId};
use genealogy_core::name::{NameType, PersonName, Surname};
use genealogy_core::person::command::{PersonCommand, PersonCommandEnvelope};
use genealogy_core::person::{PersonError, PersonView};
use genealogy_core::provenance::Confidence;
use genealogy_db::{CommandError, Store};

use crate::error::AppError;
use crate::session::Session;
use crate::workspace::Workspace;

/// A frontend-neutral summary of a person (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonSummary {
    /// The user-facing identifier (e.g. `I0001`).
    pub human_id: String,
    /// A display rendering of the primary name, if any name is asserted.
    pub display_name: Option<String>,
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
    )
    .await?;

    if new.given.is_some() || new.surname.is_some() {
        let name = build_name(new.given, new.surname);
        execute(
            store,
            session,
            &aggregate_id,
            PersonCommand::AssertName { person_id, name },
        )
        .await?;
    }

    Ok(human_id)
}

/// Asserts an additional name on an existing person, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no such person exists, [`AppError::Domain`] if the name is empty,
/// or a workspace/store error.
pub async fn add_name(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    given: Option<String>,
    surname: Option<String>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let person_id = resolve_person_id(store, human_id).await?;
    let name = build_name(given, surname);
    execute(
        store,
        session,
        &person_id.to_string(),
        PersonCommand::AssertName { person_id, name },
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

/// Executes one command through the store, mapping the command outcome to [`AppError`].
async fn execute(store: &Store, session: &Session, aggregate_id: &str, command: PersonCommand) -> Result<(), AppError> {
    let envelope = PersonCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_person(aggregate_id, envelope)
        .await
        .map_err(map_command_error)
}

/// Resolves a `human_id` to its aggregate [`PersonId`], or [`AppError::PersonNotFound`].
async fn resolve_person_id(store: &Store, human_id: &str) -> Result<PersonId, AppError> {
    let view = store
        .find_person(human_id)
        .await?
        .ok_or_else(|| AppError::PersonNotFound(human_id.to_owned()))?;
    view.person_id()
        .ok_or_else(|| AppError::PersonNotFound(human_id.to_owned()))
}

/// Builds a [`PersonName`] from optional parts; an all-empty name is rejected downstream as
/// [`PersonError::EmptyName`](genealogy_core::person::PersonError).
fn build_name(given: Option<String>, surname: Option<String>) -> PersonName {
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
    let display_name = view.names().first().map(|name| render_name(name));
    let sex = view.sex().cloned();
    PersonSummary {
        human_id,
        display_name,
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

/// Maps a [`CommandError`] to [`AppError`], keeping a domain rejection distinct from infrastructure.
fn map_command_error(error: CommandError<PersonError>) -> AppError {
    match error {
        CommandError::Rejected(domain) => AppError::Domain(domain),
        CommandError::Store(db) => AppError::Db(db),
    }
}
