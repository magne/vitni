//! `DnaTest` use-cases (ADR 0006): create (anchored to a person), set provider/kit/type/build,
//! assert haplogroup, attach note, tag, show, and list.
//!
//! Creating a test resolves the anchoring person's `human_id` to its id (an
//! [`AppError::PersonNotFound`] if absent); the core then re-checks it against the Person projection
//! via the aggregate's `Services` resolver, surfacing `DnaTestError::UnknownPerson` — the §9
//! aggregate-tax check (ADR 0004 §3).

use std::collections::{BTreeSet, HashMap};

use vitni_core::dna::{DnaGenomeBuild, DnaProvider, DnaTestType};
use vitni_core::dna_test::DnaTestView;
use vitni_core::dna_test::command::{DnaTestCommand, DnaTestCommandEnvelope};
use vitni_core::enums::Restriction;
use vitni_core::ids::{AssertionId, DnaTestId, HumanId, NoteId, PersonId, TagId};
use vitni_core::person::PersonView;
use vitni_core::provenance::EvidenceRef;
use vitni_db::Store;

use crate::citation::TagRef;
use crate::dto::{AggRef, AttachedRef, tag_refs};
use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, MutationMeta, Provenance};
use crate::workspace::Workspace;

/// A frontend-neutral summary of a DNA test (the DTO the CLI/UI renders), carrying its stable id and
/// the joined views the detail tabs render (the cross-aggregate-joins dependency note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaTestSummary {
    /// The user-facing identifier (e.g. `D0001`).
    pub human_id: String,
    /// The stable `DnaTestId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The anchoring person (its `human_id` + stable id + display name), for the Tested-person card.
    pub person: Option<AggRef>,
    /// The anchoring person's display name, if any.
    pub person_name: Option<String>,
    /// The testing provider. Structured (not a label) so the frontend localizes it (ADR 0003).
    pub provider: Option<DnaProvider>,
    /// The test type. Structured so the frontend localizes it.
    pub test_type: Option<DnaTestType>,
    /// The provider's kit id, if set.
    pub kit_id: Option<String>,
    /// The genome build. Structured so the frontend localizes it.
    pub genome_build: Option<DnaGenomeBuild>,
    /// The recorded haplogroups (the Haplogroups tab), each with the `AssertionId` that introduced it.
    pub haplogroups: Vec<HaplogroupRef>,
    /// The matches this kit produced (the Matches tab), joined to the other test.
    pub matches: Vec<DnaTestMatchRef>,
    /// The attached notes (the Notes tab), with the attach `AssertionId` (the Detach target).
    pub notes: Vec<AttachedRef>,
    /// The applied tags (the Tags tab), by name/colour/priority.
    pub tags: Vec<TagRef>,
    /// The test's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// An asserted haplogroup — one row on the DNA test › Haplogroups tab, with the `AssertionId` that
/// introduced it (the target a per-row Edit supersedes and a Remove retracts — ADR 0004 §2).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HaplogroupRef {
    /// The haplogroup value.
    pub value: String,
    /// The `AssertionId` (a UUID string) that introduced this haplogroup. Never rendered.
    pub assertion_id: String,
}

/// A match this kit produced — one row on the DNA test › Matches tab, joined to the compared test.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaTestMatchRef {
    /// The match (its `human_id` + stable id) — the navigation key.
    pub dna_match: AggRef,
    /// The other test compared against this kit (its `human_id` + stable id), if still projected.
    pub compared_test: Option<AggRef>,
    /// Total shared centimorgans, rendered for display.
    pub shared_cm: Option<String>,
    /// Shared percentage, rendered for display.
    pub percent_shared: Option<String>,
    /// The provider's predicted relationship, if any.
    pub predicted_relationship: Option<String>,
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
pub async fn create_dna_test(
    workspace: &Workspace,
    session: &Session,
    new: NewDnaTest,
    provenance: Provenance,
    citations: &[String],
) -> Result<String, AppError> {
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
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;

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
        provenance,
        citation_refs,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute_dna_test_mutation(
        store,
        session,
        dna_test_id,
        DnaTestCommand::SetProvider { dna_test_id, provider },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute_dna_test_mutation(
        store,
        session,
        dna_test_id,
        DnaTestCommand::SetKitId { dna_test_id, kit_id },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute_dna_test_mutation(
        store,
        session,
        dna_test_id,
        DnaTestCommand::SetTestType { dna_test_id, test_type },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute_dna_test_mutation(
        store,
        session,
        dna_test_id,
        DnaTestCommand::SetGenomeBuild {
            dna_test_id,
            genome_build,
        },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute_dna_test_mutation(
        store,
        session,
        dna_test_id,
        DnaTestCommand::AssertHaplogroup {
            dna_test_id,
            haplogroup,
        },
        meta,
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute_dna_test_mutation(
        store,
        session,
        dna_test_id,
        DnaTestCommand::AttachNote { dna_test_id, note_id },
        meta,
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
    tag_id: &str,
    remove: bool,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    let command = if remove {
        DnaTestCommand::Untag { dna_test_id, tag_id }
    } else {
        DnaTestCommand::Tag { dna_test_id, tag_id }
    };
    execute_dna_test_mutation(store, session, dna_test_id, command, meta).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// Attaches a note (by its `human_id`) to a test — the UI/importer-facing wrapper.
///
/// # Errors
///
/// [`AppError::DnaTestNotFound`] / [`AppError::NoteNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_attach_dna_test_note(
    workspace: &Workspace,
    session: &Session,
    test_human_id: &str,
    note_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = use_case::resolve_id(
        store.find_note(note_human_id).await?,
        vitni_core::note::NoteView::note_id,
        || AppError::NoteNotFound(note_human_id.to_owned()),
    )?;
    attach_dna_test_note(workspace, session, test_human_id, note_id, MutationMeta::default()).await
}

/// Loads a single test's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_dna_test(workspace: &Workspace, human_id: &str) -> Result<Option<DnaTestSummary>, AppError> {
    let Some(view) = workspace.store().find_dna_test(human_id).await? else {
        return Ok(None);
    };
    let lookups = DnaTestLookups::load(workspace).await?;
    Ok(Some(summarize(&view, &lookups)))
}

/// Lists every test's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_dna_tests(workspace: &Workspace) -> Result<Vec<DnaTestSummary>, AppError> {
    let views = workspace.store().list_dna_tests().await?;
    let lookups = DnaTestLookups::load(workspace).await?;
    Ok(views.iter().map(|view| summarize(view, &lookups)).collect())
}

/// The lookups `summarize` needs to join a test's anchoring person, notes, tags, and the matches it
/// produced without a per-row query (the cross-aggregate join lives here — the app/db layer).
struct DnaTestLookups {
    /// `PersonId string -> (human_id, display name)`.
    persons: HashMap<String, (String, Option<String>)>,
    /// `NoteId -> human_id`.
    notes: HashMap<vitni_core::ids::NoteId, use_case::NoteLookup>,
    /// `TagId -> TagRef`.
    tags: HashMap<TagId, TagRef>,
    /// `DnaTestId string -> human_id`, for labelling the compared test of each match.
    tests: HashMap<String, String>,
    /// Every observed match, for the per-test Matches join.
    matches: Vec<DnaMatchRow>,
}

/// The fields of a match the Matches-tab join needs (both tests + the observed totals).
struct DnaMatchRow {
    human_id: String,
    id: String,
    test_a: Option<String>,
    test_b: Option<String>,
    shared_cm: Option<String>,
    percent_shared: Option<String>,
    predicted_relationship: Option<String>,
}

impl DnaTestLookups {
    async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let store = workspace.store();
        let names: HashMap<String, Option<String>> = crate::person::list_persons(workspace)
            .await?
            .into_iter()
            .map(|p| (p.human_id, p.display_name))
            .collect();
        let mut persons = HashMap::new();
        for view in store.list_persons().await? {
            if let Some(id) = view.person_id() {
                let human_id = view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default();
                let name = names.get(&human_id).cloned().flatten();
                persons.insert(id.to_string(), (human_id, name));
            }
        }
        let mut tests = HashMap::new();
        for view in store.list_dna_tests().await? {
            if let (Some(id), Some(human_id)) = (view.dna_test_id(), view.human_id()) {
                tests.insert(id.to_string(), human_id.as_str().to_owned());
            }
        }
        let mut matches = Vec::new();
        for view in store.list_dna_matches().await? {
            let Some(id) = view.dna_match_id() else { continue };
            matches.push(DnaMatchRow {
                human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
                id: id.to_string(),
                test_a: view.test_a().map(|t| t.to_string()),
                test_b: view.test_b().map(|t| t.to_string()),
                shared_cm: view.shared_cm().map(|c| c.to_string()),
                percent_shared: view.percent_shared().map(|p| p.to_string()),
                predicted_relationship: view.predicted_relationship().map(ToOwned::to_owned),
            });
        }
        Ok(Self {
            persons,
            notes: use_case::note_lookups(store).await?,
            tags: tag_refs(store).await?,
            tests,
            matches,
        })
    }

    /// Resolves a `PersonId` string to its `human_id` + stable id reference.
    fn person_ref(&self, person_id: &str) -> Option<AggRef> {
        self.persons.get(person_id).map(|(human_id, _)| AggRef {
            human_id: human_id.clone(),
            id: person_id.to_owned(),
        })
    }

    /// Resolves a `DnaTestId` string to its `human_id` + stable id reference.
    fn test_ref(&self, test_id: &str) -> Option<AggRef> {
        self.tests.get(test_id).map(|human_id| AggRef {
            human_id: human_id.clone(),
            id: test_id.to_owned(),
        })
    }
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
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, human_id).await?;
    execute_dna_test_mutation(
        store,
        session,
        dna_test_id,
        DnaTestCommand::SetRestrictions {
            dna_test_id,
            restrictions,
        },
        meta,
    )
    .await
}

/// Sets (or changes) a DNA test's user-facing identifier, identified by its current `human_id`,
/// returning the effective new id.
///
/// A supplied non-blank `new` id is dup-checked (a collision with a *different* record is
/// [`AppError::HumanIdTaken`]); a blank/absent `new` allocates the next free id from the workspace's
/// configured format (the regenerate case).
///
/// # Errors
///
/// [`AppError::DnaTestNotFound`] if the test is unknown, [`AppError::HumanIdTaken`] if the requested
/// id is already in use, or a workspace/store error.
pub async fn set_dna_test_human_id(
    workspace: &Workspace,
    session: &Session,
    current_human_id: &str,
    new: Option<String>,
    provenance: Provenance,
) -> Result<String, AppError> {
    let store = workspace.store();
    let dna_test_id = resolve_dna_test_id(store, current_human_id).await?;
    let human_id = match use_case::requested_human_id(new) {
        Some(id) => {
            if id != current_human_id && store.find_dna_test(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_dna_test_human_id(&workspace.dna_test_id_format()?).await?,
    };
    execute(
        store,
        session,
        &dna_test_id.to_string(),
        DnaTestCommand::SetHumanId {
            dna_test_id,
            human_id: HumanId::new(&human_id),
        },
        provenance,
        Vec::new(),
    )
    .await?;
    Ok(human_id)
}

/// Executes one command through the store, stamping it with `provenance` and `citations`
/// (`EventContext.citations` — data-model §8), and maps the outcome to [`AppError`].
async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: DnaTestCommand,
    provenance: Provenance,
    citations: Vec<EvidenceRef>,
) -> Result<(), AppError> {
    let envelope = DnaTestCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_dna_test(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Executes one non-create test mutation, applying the operator-intent [`MutationMeta`]: resolves
/// the backing citations, and — when `meta.supersedes` is set — wraps `command` in a
/// [`DnaTestCommand::SupersedeAssertion`] so the new assertion replaces the named one (ADR 0004 §2).
async fn execute_dna_test_mutation(
    store: &Store,
    session: &Session,
    dna_test_id: DnaTestId,
    command: DnaTestCommand,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    let target = use_case::parse_supersedes(meta.supersedes)?;
    let command = superseded(dna_test_id, command, target);
    execute(
        store,
        session,
        &dna_test_id.to_string(),
        command,
        meta.provenance,
        citations,
    )
    .await
}

/// Wraps `command` in a [`DnaTestCommand::SupersedeAssertion`] against `target` when superseding, or
/// returns it unchanged for a plain assertion.
fn superseded(dna_test_id: DnaTestId, command: DnaTestCommand, target: Option<AssertionId>) -> DnaTestCommand {
    match target {
        Some(target) => DnaTestCommand::SupersedeAssertion {
            dna_test_id,
            target,
            replacement: Box::new(command),
        },
        None => command,
    }
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

/// Renders a [`DnaTestView`] into the frontend DTO, joining its person, notes, tags, and the matches
/// it produced via `lookups`.
fn summarize(view: &DnaTestView, lookups: &DnaTestLookups) -> DnaTestSummary {
    let id = view.dna_test_id().map(|id| id.to_string()).unwrap_or_default();
    let person_id = view.person_id().map(|id| id.to_string());
    let person = person_id.as_deref().and_then(|p| lookups.person_ref(p));
    let person_name = person_id
        .as_deref()
        .and_then(|p| lookups.persons.get(p))
        .and_then(|(_, name)| name.clone());
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
    let haplogroups = view
        .haplogroups_with_assertions()
        .iter()
        .map(|attributed| HaplogroupRef {
            value: attributed.value.clone(),
            assertion_id: attributed.assertion_id.to_string(),
        })
        .collect();
    let tags = view
        .tags()
        .into_iter()
        .filter_map(|tag_id| lookups.tags.get(&tag_id).cloned())
        .collect();
    let matches = lookups
        .matches
        .iter()
        .filter(|m| m.test_a.as_deref() == Some(id.as_str()) || m.test_b.as_deref() == Some(id.as_str()))
        .map(|m| {
            let other = if m.test_a.as_deref() == Some(id.as_str()) {
                m.test_b.as_deref()
            } else {
                m.test_a.as_deref()
            };
            DnaTestMatchRef {
                dna_match: AggRef {
                    human_id: m.human_id.clone(),
                    id: m.id.clone(),
                },
                compared_test: other.and_then(|t| lookups.test_ref(t)),
                shared_cm: m.shared_cm.clone(),
                percent_shared: m.percent_shared.clone(),
                predicted_relationship: m.predicted_relationship.clone(),
            }
        })
        .collect();
    DnaTestSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        id,
        person,
        person_name,
        provider: view.provider().cloned(),
        test_type: view.test_type(),
        kit_id: view.kit_id().map(ToOwned::to_owned),
        genome_build: view.genome_build(),
        haplogroups,
        matches,
        notes,
        tags,
        restrictions: view.restrictions().clone(),
    }
}
