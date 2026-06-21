//! `DnaMatch` use-cases (ADR 0006): observe (between two tests), add segment, assert shared
//! ancestor, confirm/reject, attach note, tag, show, and list.
//!
//! Observing a match resolves both tests' `human_id`s to ids (an [`AppError::DnaTestNotFound`] if
//! absent); the core then re-checks both against the `DnaTest` projection via the aggregate's
//! `Services` resolver, surfacing `DnaMatchError::UnknownTest` — the §9 aggregate-tax check.

use genealogy_core::dna::{Centimorgans, DnaProvider, DnaSegment, PercentShared, SharedAncestor};
use genealogy_core::dna_match::DnaMatchView;
use genealogy_core::dna_match::command::{DnaMatchCommand, DnaMatchCommandEnvelope};
use genealogy_core::dna_match::state::MatchStatus;
use genealogy_core::dna_test::DnaTestView;
use genealogy_core::ids::{DnaMatchId, DnaTestId, HumanId, NoteId, TagId};
use genealogy_core::provenance::Confidence;
use genealogy_db::Store;

use crate::error::AppError;
use crate::session::Session;
use crate::use_case;
use crate::workspace::Workspace;

/// A frontend-neutral summary of a DNA match (the DTO the CLI renders).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaMatchSummary {
    /// The user-facing identifier (e.g. `X0001`).
    pub human_id: String,
    /// Total shared centimorgans, rendered for display.
    pub shared_cm: Option<String>,
    /// The provider's predicted relationship, if any.
    pub predicted_relationship: Option<String>,
    /// The confirmation status: `confirmed`, `rejected`, or `None` (undecided).
    pub status: Option<MatchStatus>,
    /// The number of recorded segments.
    pub segment_count: usize,
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
pub async fn observe_dna_match(workspace: &Workspace, session: &Session, new: NewDnaMatch) -> Result<String, AppError> {
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
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_match_id = resolve_dna_match_id(store, human_id).await?;
    execute(
        store,
        session,
        &dna_match_id.to_string(),
        DnaMatchCommand::AddSegment { dna_match_id, segment },
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
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_match_id = resolve_dna_match_id(store, human_id).await?;
    execute(
        store,
        session,
        &dna_match_id.to_string(),
        DnaMatchCommand::AssertSharedAncestor { dna_match_id, ancestor },
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
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_match_id = resolve_dna_match_id(store, human_id).await?;
    let command = if confirmed {
        DnaMatchCommand::ConfirmMatch { dna_match_id }
    } else {
        DnaMatchCommand::RejectMatch { dna_match_id }
    };
    execute(store, session, &dna_match_id.to_string(), command).await
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
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_match_id = resolve_dna_match_id(store, human_id).await?;
    execute(
        store,
        session,
        &dna_match_id.to_string(),
        DnaMatchCommand::AttachNote { dna_match_id, note_id },
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
    tag_id: TagId,
    remove: bool,
) -> Result<(), AppError> {
    let store = workspace.store();
    let dna_match_id = resolve_dna_match_id(store, human_id).await?;
    let command = if remove {
        DnaMatchCommand::Untag { dna_match_id, tag_id }
    } else {
        DnaMatchCommand::Tag { dna_match_id, tag_id }
    };
    execute(store, session, &dna_match_id.to_string(), command).await
}

/// Loads a single match's summary by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn show_dna_match(workspace: &Workspace, human_id: &str) -> Result<Option<DnaMatchSummary>, AppError> {
    let found = workspace.store().find_dna_match(human_id).await?;
    Ok(found.as_ref().map(summarize))
}

/// Lists every match's summary, ordered by `human_id`.
///
/// # Errors
///
/// A store/read-model error.
pub async fn list_dna_matches(workspace: &Workspace) -> Result<Vec<DnaMatchSummary>, AppError> {
    let views = workspace.store().list_dna_matches().await?;
    let mut summaries = Vec::with_capacity(views.len());
    for view in &views {
        summaries.push(summarize(view));
    }
    Ok(summaries)
}

/// Executes one command through the store, mapping the command outcome to [`AppError`].
async fn execute(
    store: &Store,
    session: &Session,
    aggregate_id: &str,
    command: DnaMatchCommand,
) -> Result<(), AppError> {
    let envelope = DnaMatchCommandEnvelope {
        meta: session.new_meta(Confidence::Normal, None, Vec::new()),
        command,
    };
    store
        .execute_dna_match(aggregate_id, envelope)
        .await
        .map_err(use_case::map_command_error)
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

/// Renders a [`DnaMatchView`] into the frontend DTO.
fn summarize(view: &DnaMatchView) -> DnaMatchSummary {
    DnaMatchSummary {
        human_id: view.human_id().map(|h| h.as_str().to_owned()).unwrap_or_default(),
        shared_cm: view.shared_cm().map(|c| c.to_string()),
        predicted_relationship: view.predicted_relationship().map(ToOwned::to_owned),
        status: view.status(),
        segment_count: view.segments().len(),
    }
}
