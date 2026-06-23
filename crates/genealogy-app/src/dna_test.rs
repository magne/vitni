//! `DnaTest` use-cases (ADR 0006): create (anchored to a person), set provider/kit/type/build,
//! assert haplogroup, attach note, tag, show, and list.
//!
//! Creating a test resolves the anchoring person's `human_id` to its id (an
//! [`AppError::PersonNotFound`] if absent); the core then re-checks it against the Person projection
//! via the aggregate's `Services` resolver, surfacing `DnaTestError::UnknownPerson` — the §9
//! aggregate-tax check (ADR 0004 §3).

use std::collections::BTreeSet;

use genealogy_core::dna::{DnaGenomeBuild, DnaProvider, DnaTestType};
use genealogy_core::dna_test::DnaTestView;
use genealogy_core::dna_test::command::{DnaTestCommand, DnaTestCommandEnvelope};
use genealogy_core::enums::Restriction;
use genealogy_core::ids::{DnaTestId, HumanId, NoteId, PersonId, TagId};
use genealogy_core::person::PersonView;
use genealogy_core::provenance::Confidence;
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case;
use crate::workspace::Workspace;

/// A frontend-neutral summary of a DNA test (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaTestSummary {
    /// The user-facing identifier (e.g. `D0001`).
    pub human_id: String,
    /// The anchoring person's id (a UUID string).
    pub person: Option<String>,
    /// The testing provider. Structured (not a label) so the frontend localizes it (ADR 0003).
    pub provider: Option<DnaProvider>,
    /// The test type. Structured so the frontend localizes it.
    pub test_type: Option<DnaTestType>,
    /// The number of recorded haplogroups.
    pub haplogroup_count: usize,
    /// The test's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// What to create a DNA test with (the auto/override `human_id` and the anchoring person).
#[derive(Debug, Clone)]
pub struct NewDnaTest {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// The anchoring person's `human_id` (e.g. `I0001`).
    pub person: String,
}

/// Creates a DNA test anchored to a person, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::PersonNotFound`] if the
/// anchoring person does not exist, [`AppError::DnaTestDomain`] if a domain rule rejects the command
/// (e.g. `UnknownPerson`), or a workspace/store error.
pub async fn create_dna_test(workspace: &Workspace, session: &Session, new: NewDnaTest) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_dna_test(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_dna_test_human_id(&workspace.dna_test_id_format()?).await?,
    };

    let person_id = resolve_person_id(store, &new.person).await?;
    let dna_test_id = session.new_dna_test_id();
    execute(
        store,
        session,
        &dna_test_id.to_string(),
        DnaTestCommand::CreateDnaTest {
            dna_test_id,
            human_id: HumanId::new(&human_id),
            person_id,
        },
    )
    .await?;
    Ok(human_id)
}

/// Sets (or changes) a test's provider, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::DnaTestNotFound`] if no such test exists, or a workspace/store error.
pub async fn set_dna_test_provider(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    provider: DnaProvider,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute(
        store,
        session,
        &dna_test_id.to_string(),
        DnaTestCommand::SetProvider { dna_test_id, provider },
    )
    .await
}

/// Sets (or changes) a test's kit id, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::DnaTestNotFound`] if no such test exists, or a workspace/store error.
pub async fn set_dna_test_kit_id(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    kit_id: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute(
        store,
        session,
        &dna_test_id.to_string(),
        DnaTestCommand::SetKitId { dna_test_id, kit_id },
    )
    .await
}

/// Sets (or changes) a test's type, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::DnaTestNotFound`] if no such test exists, or a workspace/store error.
pub async fn set_dna_test_type(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    test_type: DnaTestType,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute(
        store,
        session,
        &dna_test_id.to_string(),
        DnaTestCommand::SetTestType { dna_test_id, test_type },
    )
    .await
}

/// Sets (or changes) a test's genome build, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::DnaTestNotFound`] if no such test exists, or a workspace/store error.
pub async fn set_dna_test_genome_build(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    genome_build: DnaGenomeBuild,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute(
        store,
        session,
        &dna_test_id.to_string(),
        DnaTestCommand::SetGenomeBuild {
            dna_test_id,
            genome_build,
        },
    )
    .await
}

/// Asserts a haplogroup on a test, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::DnaTestNotFound`] if no such test exists, or a workspace/store error.
pub async fn assert_dna_test_haplogroup(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    haplogroup: String,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute(
        store,
        session,
        &dna_test_id.to_string(),
        DnaTestCommand::AssertHaplogroup {
            dna_test_id,
            haplogroup,
        },
    )
    .await
}

/// Attaches a note (by note aggregate id) to a test, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::DnaTestNotFound`] if no such test exists, or a workspace/store error.
pub async fn attach_dna_test_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_id: NoteId,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute(
        store,
        session,
        &dna_test_id.to_string(),
        DnaTestCommand::AttachNote { dna_test_id, note_id },
    )
    .await
}

/// Applies (or removes) a tag on a test, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::DnaTestNotFound`] if no such test exists, or a workspace/store error.
pub async fn tag_dna_test(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: TagId,
    remove: bool,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    let command = if remove {
        DnaTestCommand::Untag { dna_test_id, tag_id }
    } else {
        DnaTestCommand::Tag { dna_test_id, tag_id }
    };
    execute(store, session, &dna_test_id.to_string(), command).await
}

/// Loads a single test's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_dna_test(workspace: &Workspace, human_id: &str) -> Result<Option<DnaTestSummary>, AppError> {
    let found = workspace.store().find_dna_test(human_id).await?;
    Ok(found.as_ref().map(summarize))
}

/// Lists every test's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_dna_tests(workspace: &Workspace) -> Result<Vec<DnaTestSummary>, AppError> {
    let views = workspace.store().list_dna_tests().await?;
    let mut summaries = Vec::with_capacity(views.len());
    for view in &views {
        summaries.push(summarize(view));
    }
    Ok(summaries)
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
/// Sets a DNA test's privacy restrictions (GEDCOM `RESN` — data-model §6).
///
/// # Errors
///
/// [`AppError::DnaTestNotFound`] if no such test exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute(
        store,
        session,
        &dna_test_id.to_string(),
        DnaTestCommand::SetRestrictions {
            dna_test_id,
            restrictions,
        },
    )
    .await
}

async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: DnaTestCommand,
) -> Result<(), AppError> {
    let envelope = DnaTestCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_dna_test(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Resolves a `human_id` to its aggregate [`DnaTestId`], or [`AppError::DnaTestNotFound`].
async fn resolve_dna_test_id(store: &Store, human_id: &str) -> Result<DnaTestId, AppError> {
    use_case::resolve_id(store.find_dna_test(human_id).await?, DnaTestView::dna_test_id, || {
        AppError::DnaTestNotFound(human_id.to_owned())
    })
}

/// Resolves a person `human_id` to its aggregate [`PersonId`], or [`AppError::PersonNotFound`].
async fn resolve_person_id(store: &Store, human_id: &str) -> Result<PersonId, AppError> {
    use_case::resolve_id(store.find_person(human_id).await?, PersonView::person_id, || {
        AppError::PersonNotFound(human_id.to_owned())
    })
}

/// Renders a [`DnaTestView`] into the frontend DTO.
fn summarize(view: &DnaTestView) -> DnaTestSummary {
    DnaTestSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        person: view.person_id().map(|id| id.to_string()),
        provider: view.provider().cloned(),
        test_type: view.test_type(),
        haplogroup_count: view.haplogroups().len(),
        restrictions: view.restrictions().clone(),
    }
}
