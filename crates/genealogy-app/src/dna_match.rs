//! `DnaMatch` use-cases (ADR 0006): observe (between two tests), add segment, assert shared
//! ancestor, confirm/reject, attach note, tag, show, and list.
//!
//! Observing a match resolves both tests' `human_id`s to ids (an [`AppError::DnaTestNotFound`] if
//! absent); the core then re-checks both against the `DnaTest` projection via the aggregate's
//! `Services` resolver, surfacing `DnaMatchError::UnknownTest` — the §9 aggregate-tax check.

use std::collections::{BTreeSet, HashMap};

use genealogy_core::dna::{Centimorgans, DnaProvider, DnaSegment, PercentShared, SharedAncestor};
use genealogy_core::dna_match::DnaMatchView;
use genealogy_core::dna_match::command::{DnaMatchCommand, DnaMatchCommandEnvelope};
use genealogy_core::dna_match::state::MatchStatus;
use genealogy_core::dna_test::DnaTestView;
use genealogy_core::enums::Restriction;
use genealogy_core::ids::{AssertionId, DnaMatchId, DnaTestId, HumanId, NoteId, TagId};
use genealogy_core::provenance::CitationRef;
use genealogy_db::Store;

use crate::citation::TagRef;
use crate::dto::{AggRef, tag_refs};
use crate::error::AppError;
use crate::session::Session;
use crate::use_case::{self, MutationMeta, Provenance};
use crate::workspace::Workspace;

/// A frontend-neutral summary of a DNA match (the DTO the CLI/UI renders), carrying its stable id and
/// the joined views the detail tabs render (the cross-aggregate-joins dependency note).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaMatchSummary {
    /// The user-facing identifier (e.g. `X0001`).
    pub human_id: String,
    /// The stable `DnaMatchId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// One side's test (its `human_id` + stable id), if still projected.
    pub test_a: Option<AggRef>,
    /// One side's tested-person name, if resolvable (the row label).
    pub test_a_person: Option<String>,
    /// The other side's test (its `human_id` + stable id), if still projected.
    pub test_b: Option<AggRef>,
    /// The other side's tested-person name, if resolvable.
    pub test_b_person: Option<String>,
    /// The provider the match was observed at.
    pub provider: Option<DnaProvider>,
    /// Total shared centimorgans, rendered for display.
    pub shared_cm: Option<String>,
    /// Shared percentage, rendered for display.
    pub percent_shared: Option<String>,
    /// The largest shared segment's length, rendered for display.
    pub largest_segment_cm: Option<String>,
    /// The provider's predicted relationship, if any.
    pub predicted_relationship: Option<String>,
    /// The confirmation status: `confirmed`, `rejected`, or `None` (undecided).
    pub status: Option<MatchStatus>,
    /// The recorded shared segments (the Segments tab).
    pub segments: Vec<DnaSegment>,
    /// The inferred shared ancestors (the Shared ancestors tab), joined to the person where resolved.
    pub shared_ancestors: Vec<SharedAncestorRef>,
    /// The attached notes (the Notes tab).
    pub notes: Vec<AggRef>,
    /// The applied tags (the Tags tab), by name/colour/priority.
    pub tags: Vec<TagRef>,
    /// The match's privacy restrictions (GEDCOM `RESN`; empty = unrestricted).
    pub restrictions: BTreeSet<Restriction>,
}

/// An inferred common ancestor — one row on the DNA match › Shared ancestors tab, joined to the
/// Person projection where the ancestor was identified in this workspace.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SharedAncestorRef {
    /// The inferred common-ancestor person (its `human_id` + stable id), if identified.
    pub person: Option<AggRef>,
    /// The ancestor's display name, if resolvable.
    pub person_name: Option<String>,
    /// The free-text note describing the shared ancestry, if any.
    pub note: Option<String>,
}

/// What to observe a match with: the two tests, provider, and the observed totals.
#[derive(Debug, Clone)]
pub struct NewDnaMatch {
    /// A caller-supplied `human_id`; `None` auto-allocates the next free one.
    pub human_id: Option<String>,
    /// One side's test `human_id` (e.g. `D0001`).
    pub test_a: String,
    /// The other side's test `human_id`.
    pub test_b: String,
    /// The provider the match was observed at.
    pub provider: DnaProvider,
    /// Total shared centimorgans.
    pub shared_cm: Centimorgans,
    /// Shared percentage, if reported.
    pub percent_shared: Option<PercentShared>,
    /// The number of shared segments.
    pub segment_count: u32,
    /// The largest shared segment's length.
    pub largest_segment_cm: Centimorgans,
    /// The provider's predicted relationship, if any.
    pub predicted_relationship: Option<String>,
}

/// Observes a match between two tests, returning the assigned `human_id`.
///
/// # Errors
///
/// [`AppError::HumanIdTaken`] if a supplied id is in use, [`AppError::DnaTestNotFound`] if either
/// test is unknown, [`AppError::DnaMatchDomain`] if a domain rule rejects the command (e.g.
/// `SameTestBothSides`, `NegativeSharedCm`, `UnknownTest`), or a workspace/store error.
pub async fn observe_dna_match(
    workspace: &Workspace,
    session: &Session,
    new: NewDnaMatch,
    provenance: Provenance,
    citations: &[String],
) -> Result<String, AppError> {
    let store = workspace.store();
    let human_id = match new.human_id {
        Some(id) => {
            if store.find_dna_match(&id).await?.is_some() {
                return Err(AppError::HumanIdTaken(id));
            }
            id
        }
        None => store.next_dna_match_human_id(&workspace.dna_match_id_format()?).await?,
    };
    let citation_refs = use_case::resolve_citation_refs(store, citations).await?;

    let test_a = resolve_dna_test_id(store, &new.test_a).await?;
    let test_b = resolve_dna_test_id(store, &new.test_b).await?;
    let dna_match_id = session.new_dna_match_id();
    execute(
        store,
        session,
        &dna_match_id.to_string(),
        DnaMatchCommand::ObserveMatch {
            dna_match_id,
            human_id: HumanId::new(&human_id),
            test_a,
            test_b,
            provider: new.provider,
            shared_cm: new.shared_cm,
            percent_shared: new.percent_shared,
            segment_count: new.segment_count,
            largest_segment_cm: new.largest_segment_cm,
            predicted_relationship: new.predicted_relationship,
        },
        provenance,
        citation_refs,
    )
    .await?;
    Ok(human_id)
}

/// Adds a shared segment to a match, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::DnaMatchNotFound`] if no such match exists, or a workspace/store error.
pub async fn add_dna_match_segment(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    segment: DnaSegment,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_match_id = resolve_dna_match_id(store, human_id).await?;
    execute_dna_match_mutation(
        store,
        session,
        dna_match_id,
        DnaMatchCommand::AddSegment { dna_match_id, segment },
        meta,
    )
    .await
}

/// Asserts an inferred shared ancestor on a match, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::DnaMatchNotFound`] if no such match exists, or a workspace/store error.
pub async fn assert_dna_match_shared_ancestor(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    ancestor: SharedAncestor,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_match_id = resolve_dna_match_id(store, human_id).await?;
    execute_dna_match_mutation(
        store,
        session,
        dna_match_id,
        DnaMatchCommand::AssertSharedAncestor { dna_match_id, ancestor },
        meta,
    )
    .await
}

/// Confirms (or rejects) a match, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::DnaMatchNotFound`] if no such match exists, or a workspace/store error.
pub async fn set_dna_match_status(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    confirmed: bool,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_match_id = resolve_dna_match_id(store, human_id).await?;
    let command = if confirmed {
        DnaMatchCommand::ConfirmMatch { dna_match_id }
    } else {
        DnaMatchCommand::RejectMatch { dna_match_id }
    };
    execute_dna_match_mutation(store, session, dna_match_id, command, meta).await
}

/// Attaches a note (by note aggregate id) to a match, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::DnaMatchNotFound`] if no such match exists, or a workspace/store error.
pub async fn attach_dna_match_note(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    note_id: NoteId,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_match_id = resolve_dna_match_id(store, human_id).await?;
    execute_dna_match_mutation(
        store,
        session,
        dna_match_id,
        DnaMatchCommand::AttachNote { dna_match_id, note_id },
        meta,
    )
    .await
}

/// Applies (or removes) a tag on a match, identified by `human_id`.
///
/// # Errors
///
/// [`AppError::DnaMatchNotFound`] if no such match exists, or a workspace/store error.
pub async fn tag_dna_match(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    tag_id: &str,
    remove: bool,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_match_id = resolve_dna_match_id(store, human_id).await?;
    let tag_id = parse_tag_id(tag_id)?;
    let command = if remove {
        DnaMatchCommand::Untag { dna_match_id, tag_id }
    } else {
        DnaMatchCommand::Tag { dna_match_id, tag_id }
    };
    execute_dna_match_mutation(store, session, dna_match_id, command, meta).await
}

/// Parses a tag's aggregate id (a UUID string) to a [`TagId`], or [`AppError::TagNotFound`].
fn parse_tag_id(id: &str) -> Result<TagId, AppError> {
    uuid::Uuid::parse_str(id)
        .map(TagId::from_uuid)
        .map_err(|_| AppError::TagNotFound(id.to_owned()))
}

/// Attaches a note (by its `human_id`) to a match — the UI/importer-facing wrapper.
///
/// # Errors
///
/// [`AppError::DnaMatchNotFound`] / [`AppError::NoteNotFound`] if either does not exist, or a
/// workspace/store error.
pub async fn import_attach_dna_match_note(
    workspace: &Workspace,
    session: &Session,
    match_human_id: &str,
    note_human_id: &str,
) -> Result<(), AppError> {
    let store = workspace.store();
    let note_id = use_case::resolve_id(
        store.find_note(note_human_id).await?,
        genealogy_core::note::NoteView::note_id,
        || AppError::NoteNotFound(note_human_id.to_owned()),
    )?;
    attach_dna_match_note(workspace, session, match_human_id, note_id, MutationMeta::default()).await
}

/// Loads a single match's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_dna_match(workspace: &Workspace, human_id: &str) -> Result<Option<DnaMatchSummary>, AppError> {
    let Some(view) = workspace.store().find_dna_match(human_id).await? else {
        return Ok(None);
    };
    let lookups = DnaMatchLookups::load(workspace).await?;
    Ok(Some(summarize(&view, &lookups)))
}

/// Lists every match's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_dna_matches(workspace: &Workspace) -> Result<Vec<DnaMatchSummary>, AppError> {
    let views = workspace.store().list_dna_matches().await?;
    let lookups = DnaMatchLookups::load(workspace).await?;
    Ok(views.iter().map(|view| summarize(view, &lookups)).collect())
}

/// The lookups `summarize` needs to join a match's compared tests (and their tested people), shared
/// ancestors, notes, and tags without a per-row query (the cross-aggregate join lives in the app/db
/// layer).
struct DnaMatchLookups {
    /// `DnaTestId string -> (test human_id, tested-person name)`.
    tests: HashMap<String, (String, Option<String>)>,
    /// `PersonId string -> (human_id, display name)`.
    persons: HashMap<String, (String, Option<String>)>,
    /// `NoteId -> human_id`.
    notes: HashMap<NoteId, String>,
    /// `TagId -> TagRef`.
    tags: HashMap<TagId, TagRef>,
}

impl DnaMatchLookups {
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
            if let Some(id) = view.dna_test_id() {
                let human_id = view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default();
                let person_name = view
                    .person_id()
                    .and_then(|p| persons.get(&p.to_string()))
                    .and_then(|(_, name)| name.clone());
                tests.insert(id.to_string(), (human_id, person_name));
            }
        }
        Ok(Self {
            tests,
            persons,
            notes: use_case::note_human_ids(store).await?,
            tags: tag_refs(store).await?,
        })
    }

    /// Resolves a `DnaTestId` string to a (test ref, tested-person name) pair.
    fn test_ref(&self, test_id: Option<DnaTestId>) -> (Option<AggRef>, Option<String>) {
        let Some(test_id) = test_id else {
            return (None, None);
        };
        let key = test_id.to_string();
        match self.tests.get(&key) {
            Some((human_id, person_name)) => (
                Some(AggRef {
                    human_id: human_id.clone(),
                    id: key,
                }),
                person_name.clone(),
            ),
            None => (None, None),
        }
    }
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
/// Sets a DNA match's privacy restrictions (GEDCOM `RESN` — data-model §6).
///
/// # Errors
///
/// [`AppError::DnaMatchNotFound`] if no such match exists, or a workspace/store error.
pub async fn set_restrictions(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    restrictions: BTreeSet<Restriction>,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_match_id = resolve_dna_match_id(store, human_id).await?;
    execute_dna_match_mutation(
        store,
        session,
        dna_match_id,
        DnaMatchCommand::SetRestrictions {
            dna_match_id,
            restrictions,
        },
        meta,
    )
    .await
}

/// Executes one command through the store, stamping it with `provenance` and `citations`
/// (`EventContext.citations` — data-model §8), and maps the outcome to [`AppError`].
async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: DnaMatchCommand,
    provenance: Provenance,
    citations: Vec<CitationRef>,
) -> Result<(), AppError> {
    let envelope = DnaMatchCommandEnvelope {
        meta: session.new_meta(provenance, citations),
        command,
    };
    store
        .execute_dna_match(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
}

/// Executes one non-create match mutation, applying the operator-intent [`MutationMeta`]: resolves
/// the backing citations, and — when `meta.supersedes` is set — wraps `command` in a
/// [`DnaMatchCommand::SupersedeAssertion`] so the new assertion replaces the named one (ADR 0004 §2).
async fn execute_dna_match_mutation(
    store: &Store,
    session: &Session,
    dna_match_id: DnaMatchId,
    command: DnaMatchCommand,
    meta: MutationMeta<'_>,
) -> Result<(), AppError> {
    let citations = use_case::resolve_citation_refs(store, meta.citations).await?;
    let target = use_case::parse_supersedes(meta.supersedes)?;
    let command = superseded(dna_match_id, command, target);
    execute(
        store,
        session,
        &dna_match_id.to_string(),
        command,
        meta.provenance,
        citations,
    )
    .await
}

/// Wraps `command` in a [`DnaMatchCommand::SupersedeAssertion`] against `target` when superseding, or
/// returns it unchanged for a plain assertion.
fn superseded(dna_match_id: DnaMatchId, command: DnaMatchCommand, target: Option<AssertionId>) -> DnaMatchCommand {
    match target {
        Some(target) => DnaMatchCommand::SupersedeAssertion {
            dna_match_id,
            target,
            replacement: Box::new(command),
        },
        None => command,
    }
}

/// Resolves a `human_id` to its aggregate [`DnaMatchId`], or [`AppError::DnaMatchNotFound`].
async fn resolve_dna_match_id(store: &Store, human_id: &str) -> Result<DnaMatchId, AppError> {
    use_case::resolve_id(
        store.find_dna_match(human_id).await?,
        DnaMatchView::dna_match_id,
        || AppError::DnaMatchNotFound(human_id.to_owned()),
    )
}

/// Resolves a test `human_id` to its aggregate [`DnaTestId`], or [`AppError::DnaTestNotFound`].
async fn resolve_dna_test_id(store: &Store, human_id: &str) -> Result<DnaTestId, AppError> {
    use_case::resolve_id(store.find_dna_test(human_id).await?, DnaTestView::dna_test_id, || {
        AppError::DnaTestNotFound(human_id.to_owned())
    })
}

/// Renders a [`DnaMatchView`] into the frontend DTO, joining its compared tests, shared ancestors,
/// notes, and tags via `lookups`.
fn summarize(view: &DnaMatchView, lookups: &DnaMatchLookups) -> DnaMatchSummary {
    let (test_a, test_a_person) = lookups.test_ref(view.test_a());
    let (test_b, test_b_person) = lookups.test_ref(view.test_b());
    let segments = view.segments().into_iter().cloned().collect();
    let shared_ancestors = view
        .shared_ancestors()
        .into_iter()
        .map(|ancestor| {
            let person_id = ancestor.ancestor_person_id.map(|id| id.to_string());
            let person = person_id.as_deref().and_then(|p| {
                lookups.persons.get(p).map(|(human_id, _)| AggRef {
                    human_id: human_id.clone(),
                    id: p.to_owned(),
                })
            });
            let person_name = person_id
                .as_deref()
                .and_then(|p| lookups.persons.get(p))
                .and_then(|(_, name)| name.clone());
            SharedAncestorRef {
                person,
                person_name,
                note: ancestor.note.clone(),
            }
        })
        .collect();
    let notes = view
        .notes()
        .into_iter()
        .filter_map(|note_id| {
            lookups.notes.get(&note_id).map(|human_id| AggRef {
                human_id: human_id.clone(),
                id: note_id.to_string(),
            })
        })
        .collect();
    let tags = view
        .tags()
        .into_iter()
        .filter_map(|tag_id| lookups.tags.get(&tag_id).cloned())
        .collect();
    DnaMatchSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        id: view.dna_match_id().map(|id| id.to_string()).unwrap_or_default(),
        test_a,
        test_a_person,
        test_b,
        test_b_person,
        provider: view.provider().cloned(),
        shared_cm: view.shared_cm().map(|c| c.to_string()),
        percent_shared: view.percent_shared().map(|p| p.to_string()),
        largest_segment_cm: view.largest_segment_cm().map(|c| c.to_string()),
        predicted_relationship: view.predicted_relationship().map(ToOwned::to_owned),
        status: view.status(),
        segments,
        shared_ancestors,
        notes,
        tags,
        restrictions: view.restrictions().clone(),
    }
}
