//! Import-run use-cases (ADR 0037 §3, §5): start, finish and abandon a run, list runs, and the
//! datasets projection over them.
//!
//! A run's operator is the invoking human, so these take that human's [`Session`], never the
//! importer's `Software` one. Datasets are not stored on their own: a dataset is the set of runs that
//! name it, labelled by its earliest run.

use vitni_core::ids::ImportRunId;
use vitni_core::import_run::{
    AbandonReason, ImportCounts, ImportRunCommand, ImportRunCommandEnvelope, ImportRunStatus, ImportRunView,
    NewImportRun, ResolvedItem,
};
use vitni_core::origin::{DatasetId, DatasetScope, DatasetSpec};
use vitni_core::provenance::Timestamp;

use crate::error::AppError;
use crate::session::Session;
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
    /// Who started the run, if the operator has a display name.
    pub operator_display: Option<String>,
    /// When the run started.
    pub started_at: Timestamp,
    /// Where the run is in its life.
    pub status: ImportRunStatus,
    /// What the run wrote (zero until it has ended).
    pub counts: ImportCounts,
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
    let mut runs = Vec::with_capacity(views.len());
    for view in &views {
        if let Some(run) = summarize(view) {
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
        if let Some(dataset) = datasets.iter_mut().find(|dataset| dataset.id == run.dataset) {
            dataset.runs += 1;
        } else {
            datasets.push(DatasetSummary {
                id: run.dataset,
                label: run.dataset_label,
                runs: 1,
            });
        }
    }
    Ok(datasets)
}

/// Resolves the operator's dataset `choice` for an importer writing into `spec`.
///
/// A global dataset is never chosen. For a lineage importer, no choice means a new dataset when none
/// of its scheme exists yet, and is refused when one does: guessing would silently merge two files'
/// records (ADR 0037 §3). A new lineage is labelled `source_label`.
///
/// # Errors
///
/// [`AppError::Dataset`] when the choice names no dataset, names more than one, is missing while
/// datasets exist, or asks for a lineage of a global importer; or a store error.
pub async fn choose_dataset(
    workspace: &Workspace,
    session: &Session,
    spec: &DatasetSpec,
    choice: DatasetChoice,
    source_label: &str,
) -> Result<ChosenDataset, AppError> {
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
            DatasetChoice::Unspecified => {
                let candidates = existing
                    .iter()
                    .map(|dataset| format!("{} ({})", dataset.label, dataset.id))
                    .collect();
                return Err(DatasetError::Required {
                    scheme: scheme.to_owned(),
                    candidates,
                }
                .into());
            }
        },
    };
    Ok(chosen)
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
        operator_display: view.operator().and_then(|agent| agent.display.clone()),
        started_at,
        status: view.status().clone(),
        counts: view.counts().clone(),
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
