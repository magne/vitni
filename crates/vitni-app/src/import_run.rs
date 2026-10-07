//! Import-run use-cases (ADR 0037 §3, §5): start, finish and abandon a run, list runs, the
//! datasets projection over them, and the dataset a file is proposed to belong to.
//!
//! A run's operator is the invoking human, so these take that human's [`Session`], never the
//! importer's `Software` one. Datasets are not stored on their own: a dataset is the set of runs that
//! name it, labelled by its earliest run.

use std::collections::HashMap;
use std::path::PathBuf;

use vitni_core::ids::ImportRunId;
use vitni_core::import_run::{
    AbandonReason, ImportCounts, ImportRunCommand, ImportRunCommandEnvelope, ImportRunStatus, ImportRunView,
    NewImportRun, ResolvedItem,
};
use vitni_core::matching::MatchableKind;
use vitni_core::origin::{DatasetId, DatasetScope, DatasetSpec};
use vitni_core::provenance::Timestamp;

use crate::error::AppError;
use crate::session::Session;
use crate::staging::{EntityFields, RecordGraph};
use crate::use_case::{self, Provenance};
use crate::workspace::Workspace;

/// A frontend-neutral summary of one import run.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRunSummary {
    /// The run's aggregate id.
    pub id: ImportRunId,
    /// The importing plugin's id.
    pub plugin: String,
    /// The importing plugin's version.
    pub plugin_version: String,
    /// The dataset the run's records belong to.
    pub dataset: DatasetId,
    /// The dataset's label as this run recorded it.
    pub dataset_label: String,
    /// What was imported.
    pub source_label: String,
    /// Where a bulk import read its file from, if the run recorded it: what *Resume* re-reads
    /// (ADR 0040 §5).
    pub source_path: Option<PathBuf>,
    /// The document header's fingerprint, if the importer declared one.
    pub dataset_hint: Option<String>,
    /// Who started the run, if the operator has a display name.
    pub operator_display: Option<String>,
    /// When the run started.
    pub started_at: Timestamp,
    /// Where the run is in its life.
    pub status: ImportRunStatus,
    /// What the run wrote (zero until it has ended).
    pub counts: ImportCounts,
    /// How to resume the run, when it is a bulk import that was abandoned (ADR 0040 §5).
    pub resume: Option<RunResume>,
}

/// What resuming an abandoned bulk run re-runs: its plugin over its file, into its dataset (ADR 0040
/// §5). Every record the run already wrote resolves as unchanged, so the re-run writes only the rest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunResume {
    /// The importing plugin's id.
    pub plugin: String,
    /// The dataset the run's records belong to.
    pub dataset: DatasetId,
    /// The file the run read.
    pub source: PathBuf,
}

/// How to resume each run in `views` that can be resumed (ADR 0040 §5): one abandoned after reading
/// a file, for a reason a re-run need not repeat, and still the newest run of its dataset. A later run
/// — finished, running or itself abandoned — supersedes it.
pub(crate) fn resumable(views: &[ImportRunView]) -> HashMap<ImportRunId, RunResume> {
    let mut resumable = HashMap::new();
    for view in views {
        let (ImportRunStatus::Abandoned { reason }, Some(id), Some(dataset), Some(source), Some(started_at)) = (
            view.status(),
            view.run_id(),
            view.dataset(),
            view.source_path(),
            view.started_at(),
        ) else {
            continue;
        };
        let superseded = views
            .iter()
            .any(|later| later.dataset() == Some(dataset) && later.started_at().is_some_and(|at| at > started_at));
        if !superseded && rerun_may_finish(reason) {
            let resume = RunResume {
                plugin: view.plugin().to_owned(),
                dataset: dataset.clone(),
                source: PathBuf::from(source),
            };
            resumable.insert(id, resume);
        }
    }
    resumable
}

/// Whether re-running a run abandoned for `reason` can get further: not when the importer rejected
/// the file or exhausted its budget, which the same file does again.
fn rerun_may_finish(reason: &AbandonReason) -> bool {
    match reason {
        AbandonReason::Cancelled | AbandonReason::Runtime { .. } | AbandonReason::Commit { .. } => true,
        AbandonReason::GuestError { .. } | AbandonReason::ResourceLimit => false,
    }
}

/// One dataset: the runs that name it, labelled by the earliest (ADR 0037 §5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetSummary {
    /// The dataset id.
    pub id: DatasetId,
    /// The label its first run recorded.
    pub label: String,
    /// How many runs have imported into it.
    pub runs: usize,
    /// Every distinct header fingerprint its runs declared, in the order first seen.
    pub hints: Vec<String>,
}

/// How a file's header fingerprint compares with the ones a dataset's runs declared (ADR 0037 §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Fingerprint {
    /// One of the dataset's runs declared the file's fingerprint.
    Same,
    /// The dataset's runs declared fingerprints, none of them the file's.
    Different,
    /// The file or the dataset has no fingerprint to compare.
    Unknown,
}

/// An earlier dataset a file may belong to, with the evidence for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetCandidate {
    /// The dataset id.
    pub id: DatasetId,
    /// The label its first run recorded.
    pub label: String,
    /// How many of the file's person and family records the dataset already holds.
    pub shared: usize,
    /// How the file's fingerprint compares with the dataset's.
    pub fingerprint: Fingerprint,
}

/// Which earlier dataset a lineage file is proposed to belong to (ADR 0037 §3): the tool proposes,
/// the operator decides.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DatasetProposal {
    /// How many person and family records the file holds.
    pub keys: usize,
    /// Every dataset of the importer's scheme, in the order its first run started.
    pub candidates: Vec<DatasetCandidate>,
    /// The candidate the evidence points to, if it points to one.
    pub proposed: Option<DatasetId>,
}

impl DatasetProposal {
    /// The proposed candidate, if there is one.
    #[must_use]
    pub fn proposed_candidate(&self) -> Option<&DatasetCandidate> {
        let proposed = self.proposed.as_ref()?;
        self.candidates.iter().find(|candidate| candidate.id == *proposed)
    }
}

/// Which dataset an import should write into, as the operator expressed it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DatasetChoice {
    /// No choice made: allowed only when there is nothing to choose between.
    Unspecified,
    /// An existing dataset, by id or by label.
    Existing(String),
    /// A new file lineage.
    New,
}

/// The dataset an import resolved to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChosenDataset {
    /// The dataset id.
    pub id: DatasetId,
    /// Its label: an existing dataset's own, or the source label for a new one.
    pub label: String,
}

/// Why a dataset choice could not be resolved (ADR 0037 §3).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum DatasetError {
    /// No dataset of the importer's scheme has this id or label.
    #[error("no {scheme} dataset matches {query:?}")]
    NotFound {
        /// The importer's dataset scheme.
        scheme: String,
        /// What the operator asked for.
        query: String,
    },
    /// More than one dataset has this label.
    #[error("{query:?} names more than one dataset: {candidates:?}")]
    Ambiguous {
        /// What the operator asked for.
        query: String,
        /// The ids that carry the label.
        candidates: Vec<String>,
    },
    /// Datasets of this scheme exist, so the operator must pick one or declare a new one.
    #[error("choose a {scheme} dataset: {candidates:?}")]
    Required {
        /// The importer's dataset scheme.
        scheme: String,
        /// `label (id)` for each existing dataset.
        candidates: Vec<String>,
    },
    /// The importer's record ids are global, so it has exactly one dataset and no lineages.
    #[error("{scheme} has a single global dataset")]
    Global {
        /// The importer's dataset scheme.
        scheme: String,
    },
}

/// Starts a run as `session`'s operator, returning its new id.
///
/// # Errors
///
/// A workspace/store error.
pub async fn start_import_run(
    workspace: &Workspace,
    session: &Session,
    run: NewImportRun,
) -> Result<ImportRunId, AppError> {
    let run_id = session.new_import_run_id();
    execute(
        workspace,
        session,
        run_id,
        ImportRunCommand::StartImportRun { run_id, run },
    )
    .await?;
    Ok(run_id)
}

/// Ends a run that completed, recording the resolutions it made that created nothing.
///
/// # Errors
///
/// [`AppError::ImportRunDomain`] if the run never started or has already ended, or a store error.
pub async fn finish_import_run(
    workspace: &Workspace,
    session: &Session,
    run_id: ImportRunId,
    resolved: Vec<ResolvedItem>,
    counts: ImportCounts,
) -> Result<(), AppError> {
    let command = ImportRunCommand::FinishImportRun {
        run_id,
        resolved,
        counts,
    };
    execute(workspace, session, run_id, command).await
}

/// Ends a run that did not complete.
///
/// # Errors
///
/// [`AppError::ImportRunDomain`] if the run never started or has already ended, or a store error.
pub async fn abandon_import_run(
    workspace: &Workspace,
    session: &Session,
    run_id: ImportRunId,
    resolved: Vec<ResolvedItem>,
    counts: ImportCounts,
    reason: AbandonReason,
) -> Result<(), AppError> {
    let command = ImportRunCommand::AbandonImportRun {
        run_id,
        resolved,
        counts,
        reason,
    };
    execute(workspace, session, run_id, command).await
}

/// Every import run, oldest first.
///
/// # Errors
///
/// A store error.
pub async fn list_import_runs(workspace: &Workspace) -> Result<Vec<ImportRunSummary>, AppError> {
    let views = workspace.store().list_import_runs().await?;
    let mut resumable = resumable(&views);
    let mut runs = Vec::with_capacity(views.len());
    for view in &views {
        if let Some(mut run) = summarize(view) {
            run.resume = resumable.remove(&run.id);
            runs.push(run);
        }
    }
    runs.sort_by_key(|run| (run.started_at, run.id));
    Ok(runs)
}

/// One import run by id, or `None` if no run has that id.
///
/// # Errors
///
/// A store error.
pub async fn find_import_run(workspace: &Workspace, run_id: ImportRunId) -> Result<Option<ImportRunSummary>, AppError> {
    let view = workspace.store().find_import_run(&run_id.to_string()).await?;
    Ok(view.as_ref().and_then(summarize))
}

/// Every dataset, in the order its first run started.
///
/// # Errors
///
/// A store error.
pub async fn list_datasets(workspace: &Workspace) -> Result<Vec<DatasetSummary>, AppError> {
    let runs = list_import_runs(workspace).await?;
    let mut datasets: Vec<DatasetSummary> = Vec::new();
    for run in runs {
        let index = if let Some(index) = datasets.iter().position(|dataset| dataset.id == run.dataset) {
            index
        } else {
            datasets.push(DatasetSummary {
                id: run.dataset,
                label: run.dataset_label,
                runs: 0,
                hints: Vec::new(),
            });
            datasets.len() - 1
        };
        let dataset = &mut datasets[index];
        dataset.runs += 1;
        if let Some(hint) = run.dataset_hint
            && !dataset.hints.contains(&hint)
        {
            dataset.hints.push(hint);
        }
    }
    Ok(datasets)
}

/// Proposes which earlier dataset of `spec`'s scheme a lineage file belongs to (ADR 0037 §3), from
/// the file's header fingerprint `hint` and how many of its person and family records (`graphs`) each
/// dataset already holds. A global importer has no lineages, so it is proposed nothing.
///
/// # Errors
///
/// A store error.
pub async fn propose_dataset(
    workspace: &Workspace,
    spec: &DatasetSpec,
    hint: Option<&str>,
    graphs: &[RecordGraph],
) -> Result<DatasetProposal, AppError> {
    let mut persons = Vec::new();
    let mut families = Vec::new();
    for graph in graphs {
        for entity in &graph.entities {
            if entity.item.is_some() {
                continue;
            }
            match &entity.fields {
                EntityFields::Person(_) => persons.push(graph.record.clone()),
                EntityFields::Family(_) => families.push(graph.record.clone()),
                EntityFields::Event(_)
                | EntityFields::Place(_)
                | EntityFields::Source(_)
                | EntityFields::Citation(_)
                | EntityFields::Media(_)
                | EntityFields::Note(_)
                | EntityFields::Repository(_)
                | EntityFields::Tag(_) => {}
            }
        }
    }
    let keys = persons.len() + families.len();
    if spec.scope == DatasetScope::Global {
        return Ok(DatasetProposal {
            keys,
            candidates: Vec::new(),
            proposed: None,
        });
    }
    let store = workspace.store();
    let mut overlap = store.origin_overlap(MatchableKind::Person.as_str(), &persons).await?;
    overlap.extend(store.origin_overlap(MatchableKind::Family.as_str(), &families).await?);
    let mut candidates = Vec::new();
    for dataset in list_datasets(workspace).await? {
        if dataset.id.scheme() != spec.scheme {
            continue;
        }
        let mut shared = 0;
        for (id, count) in &overlap {
            if *id == dataset.id {
                shared += count;
            }
        }
        candidates.push(DatasetCandidate {
            fingerprint: fingerprint(hint, &dataset.hints),
            id: dataset.id,
            label: dataset.label,
            shared,
        });
    }
    let proposed = propose(&candidates);
    Ok(DatasetProposal {
        keys,
        candidates,
        proposed,
    })
}

fn fingerprint(hint: Option<&str>, hints: &[String]) -> Fingerprint {
    match hint {
        None => Fingerprint::Unknown,
        Some(_) if hints.is_empty() => Fingerprint::Unknown,
        Some(hint) if hints.iter().any(|known| known == hint) => Fingerprint::Same,
        Some(_) => Fingerprint::Different,
    }
}

/// The candidate the file belongs to, if the evidence is clear: one that holds some of the file's
/// records under the file's own fingerprint. Record ids are file-local (a GEDCOM `I1` recurs in
/// unrelated files), so shared ids without a matching fingerprint propose nothing. Of the qualifying
/// candidates the larger overlap wins; a tie proposes nothing.
fn propose(candidates: &[DatasetCandidate]) -> Option<DatasetId> {
    let mut best: Option<&DatasetCandidate> = None;
    let mut tied = false;
    for candidate in candidates {
        let qualifies = candidate.shared > 0
            && match candidate.fingerprint {
                Fingerprint::Same => true,
                Fingerprint::Unknown | Fingerprint::Different => false,
            };
        if !qualifies {
            continue;
        }
        match best {
            Some(current) if current.shared > candidate.shared => {}
            Some(current) if current.shared == candidate.shared => tied = true,
            _ => {
                best = Some(candidate);
                tied = false;
            }
        }
    }
    if tied {
        return None;
    }
    best.map(|candidate| candidate.id.clone())
}

/// Resolves the operator's dataset `choice` for an importer writing into `spec`.
///
/// A global dataset is never chosen. For a lineage importer, no choice means a new dataset when none
/// of its scheme exists yet, and `None` when one does: guessing would silently merge two files'
/// records, so the file's own evidence proposes one ([`propose_dataset`]) for the operator to
/// confirm (ADR 0037 §3). A new lineage is labelled `source_label`.
///
/// # Errors
///
/// [`AppError::Dataset`] when the choice names no dataset, names more than one, or asks for a lineage
/// of a global importer; or a store error.
pub async fn choose_dataset(
    workspace: &Workspace,
    session: &Session,
    spec: &DatasetSpec,
    choice: DatasetChoice,
    source_label: &str,
) -> Result<Option<ChosenDataset>, AppError> {
    let scheme = spec.scheme.as_str();
    let existing: Vec<DatasetSummary> = list_datasets(workspace)
        .await?
        .into_iter()
        .filter(|dataset| dataset.id.scheme() == scheme)
        .collect();
    let chosen = match spec.scope {
        DatasetScope::Global => choose_global(scheme, &existing, choice)?,
        DatasetScope::Lineage => match choice {
            DatasetChoice::New => ChosenDataset {
                id: DatasetId::lineage(scheme, session.new_dataset_lineage()),
                label: source_label.to_owned(),
            },
            DatasetChoice::Existing(query) => find_dataset(scheme, &existing, &query)?,
            DatasetChoice::Unspecified if existing.is_empty() => ChosenDataset {
                id: DatasetId::lineage(scheme, session.new_dataset_lineage()),
                label: source_label.to_owned(),
            },
            DatasetChoice::Unspecified => return Ok(None),
        },
    };
    Ok(Some(chosen))
}

/// The refusal of an import that names no dataset while `proposal`'s candidates exist.
#[must_use]
pub fn dataset_required(spec: &DatasetSpec, proposal: &DatasetProposal) -> DatasetError {
    let mut candidates = Vec::with_capacity(proposal.candidates.len());
    for candidate in &proposal.candidates {
        candidates.push(format!("{} ({})", candidate.label, candidate.id));
    }
    DatasetError::Required {
        scheme: spec.scheme.clone(),
        candidates,
    }
}

fn choose_global(
    scheme: &str,
    existing: &[DatasetSummary],
    choice: DatasetChoice,
) -> Result<ChosenDataset, DatasetError> {
    let id = DatasetId::global(scheme);
    match choice {
        DatasetChoice::Unspecified => {}
        DatasetChoice::Existing(query) if query == id.as_str() => {}
        DatasetChoice::Existing(_) | DatasetChoice::New => {
            return Err(DatasetError::Global {
                scheme: scheme.to_owned(),
            });
        }
    }
    let label = existing
        .iter()
        .find(|dataset| dataset.id == id)
        .map_or_else(|| scheme.to_owned(), |dataset| dataset.label.clone());
    Ok(ChosenDataset { id, label })
}

/// The dataset whose id is `query`, else the one dataset labelled `query`.
fn find_dataset(scheme: &str, existing: &[DatasetSummary], query: &str) -> Result<ChosenDataset, DatasetError> {
    if let Some(dataset) = existing.iter().find(|dataset| dataset.id.as_str() == query) {
        return Ok(ChosenDataset {
            id: dataset.id.clone(),
            label: dataset.label.clone(),
        });
    }
    let labelled: Vec<&DatasetSummary> = existing.iter().filter(|dataset| dataset.label == query).collect();
    match labelled.as_slice() {
        [] => Err(DatasetError::NotFound {
            scheme: scheme.to_owned(),
            query: query.to_owned(),
        }),
        [dataset] => Ok(ChosenDataset {
            id: dataset.id.clone(),
            label: dataset.label.clone(),
        }),
        [..] => Err(DatasetError::Ambiguous {
            query: query.to_owned(),
            candidates: labelled.iter().map(|dataset| dataset.id.to_string()).collect(),
        }),
    }
}

fn summarize(view: &ImportRunView) -> Option<ImportRunSummary> {
    let (Some(id), Some(dataset), Some(started_at)) = (view.run_id(), view.dataset(), view.started_at()) else {
        return None;
    };
    Some(ImportRunSummary {
        id,
        plugin: view.plugin().to_owned(),
        plugin_version: view.plugin_version().to_owned(),
        dataset: dataset.clone(),
        dataset_label: view.dataset_label().to_owned(),
        source_label: view.source_label().to_owned(),
        source_path: view.source_path().map(PathBuf::from),
        dataset_hint: view.dataset_hint().map(str::to_owned),
        operator_display: view.operator().and_then(|agent| agent.display.clone()),
        started_at,
        status: view.status().clone(),
        counts: view.counts().clone(),
        resume: None,
    })
}

async fn execute(
    workspace: &Workspace,
    session: &Session,
    run_id: ImportRunId,
    command: ImportRunCommand,
) -> Result<(), AppError> {
    let envelope = ImportRunCommandEnvelope {
        meta: session.new_meta(Provenance::default(), Vec::new()),
        command,
    };
    workspace
        .store()
        .execute_import_run(&run_id.to_string(), envelope)
        .await
        .map_err(use_case::map_command_error)
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;
    use uuid::Uuid;
    use vitni_core::origin::DatasetId;

    use super::{DatasetCandidate, Fingerprint, propose};

    fn candidate(index: u128, shared: usize, fingerprint: Fingerprint) -> DatasetCandidate {
        DatasetCandidate {
            id: DatasetId::lineage("gedcom", Uuid::from_u128(index)),
            label: format!("tree-{index}.ged"),
            shared,
            fingerprint,
        }
    }

    fn qualifies(candidate: &DatasetCandidate) -> bool {
        candidate.shared > 0 && candidate.fingerprint == Fingerprint::Same
    }

    #[test]
    fn two_equally_strong_candidates_propose_neither() {
        let candidates = [candidate(1, 3, Fingerprint::Same), candidate(2, 3, Fingerprint::Same)];
        assert_eq!(propose(&candidates), None);
    }

    proptest! {
        #[test]
        fn a_proposal_qualifies_and_nothing_qualifying_outranks_or_ties_it(
            specs in prop::collection::vec((0usize..40, 0u8..3), 0..6),
        ) {
            let mut candidates = Vec::new();
            for (index, (shared, fingerprint)) in specs.into_iter().enumerate() {
                let fingerprint = match fingerprint {
                    0 => Fingerprint::Same,
                    1 => Fingerprint::Different,
                    _ => Fingerprint::Unknown,
                };
                let index = u128::try_from(index).unwrap_or(u128::MAX);
                candidates.push(candidate(index, shared, fingerprint));
            }
            let qualifying: Vec<&DatasetCandidate> = candidates.iter().filter(|candidate| qualifies(candidate)).collect();
            if let Some(id) = propose(&candidates) {
                let chosen = candidates.iter().find(|candidate| candidate.id == id);
                prop_assert!(chosen.is_some_and(qualifies));
                for other in &qualifying {
                    if other.id != id {
                        prop_assert!(chosen.is_some_and(|chosen| chosen.shared > other.shared));
                    }
                }
            } else {
                let best = qualifying.iter().map(|candidate| candidate.shared).max();
                let at_best = qualifying.iter().filter(|candidate| Some(candidate.shared) == best).count();
                prop_assert!(qualifying.is_empty() || at_best > 1, "{candidates:?}");
            }
        }
    }
}
