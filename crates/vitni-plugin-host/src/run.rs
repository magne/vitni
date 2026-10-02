//! The import run an import invocation writes (ADR 0037 §5): who started it, over which dataset, and
//! what the run has written so far.
//!
//! The dataset is chosen before the import starts, unless the operator named none while datasets of
//! the importer's scheme exist. Then the host reads the file, proposes the dataset its fingerprint and
//! records point to (ADR 0037 §3), and the frontend's [`ConfirmDataset`] returns the operator's decision.
//!
//! The run is started lazily, by the first write the origin gate lets through (`vitni_app`'s
//! [`PendingRun`]), so an invocation that writes nothing (an assisted session the operator closes at
//! once, an importer that fails while parsing, a re-import of a file already on record) leaves no run
//! behind. It is closed once the guest returns, finished or abandoned.

use std::fmt;
use std::pin::Pin;
use std::sync::Arc;

use vitni_app::{
    AbandonReason, ChosenDataset, CommitOutcome, DatasetChoice, DatasetProposal, DatasetSpec, ImportCounts,
    NewImportRun, PendingRun, ResolvedItem, Session, Timestamp, Workspace,
};

use crate::error::PluginError;
use crate::review::PlanReviewer;

/// The operator's answer to a dataset proposal: the dataset the file belongs to, or `None` to cancel
/// the import.
pub type ConfirmDataset =
    Box<dyn FnOnce(DatasetProposal) -> Pin<Box<dyn Future<Output = Option<DatasetChoice>> + Send>> + Send>;

/// The dataset an import's records belong to.
pub enum RunDataset {
    /// Chosen before the import started: named by the operator, a scheme's first import, or a global
    /// dataset.
    Chosen(ChosenDataset),
    /// The operator named none while datasets of the importer's scheme exist (ADR 0037 §3): once the
    /// file is read, the host proposes one and `confirm` decides. Bulk imports only.
    Propose {
        /// The importer's dataset scheme and scope.
        spec: DatasetSpec,
        /// Asks the operator to confirm the proposal.
        confirm: ConfirmDataset,
    },
}

impl fmt::Debug for RunDataset {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Chosen(chosen) => formatter.debug_tuple("Chosen").field(chosen).finish(),
            Self::Propose { spec, confirm: _ } => formatter.debug_struct("Propose").field("spec", spec).finish(),
        }
    }
}

/// What an import invocation's run is over, supplied by the frontend that starts the import.
#[derive(Debug)]
pub struct ImportRunSpec {
    /// The invoking human's session: the run's operator, while every imported assertion keeps the
    /// importer's `Software` agent (ADR 0037 §5).
    pub operator: Session,
    /// The dataset the run's records belong to.
    pub dataset: RunDataset,
    /// What is being imported: a file name, or an assisted session's request.
    pub source_label: String,
    /// The importing plugin's id.
    pub plugin: String,
    /// The importing plugin's own version, from its manifest.
    pub plugin_version: String,
    /// Shows a bulk import's plan and reviews its possible matches (ADR 0040 §4). An assisted import
    /// asks through its [`Presenter`](crate::Presenter) instead and ignores it.
    pub reviewer: Box<dyn PlanReviewer>,
}

impl ImportRunSpec {
    /// Splits off the dataset decision and the reviewer, leaving what describes the run.
    pub(crate) fn into_parts(self) -> (RunDataset, RunTemplate, Box<dyn PlanReviewer>) {
        let Self {
            operator,
            dataset,
            source_label,
            plugin,
            plugin_version,
            reviewer,
        } = self;
        let template = RunTemplate {
            operator,
            source_label,
            plugin,
            plugin_version,
        };
        (dataset, template, reviewer)
    }
}

/// An [`ImportRunSpec`] without its dataset: what a run is, once its dataset is decided.
#[derive(Debug)]
pub(crate) struct RunTemplate {
    operator: Session,
    source_label: String,
    plugin: String,
    plugin_version: String,
}

impl RunTemplate {
    /// The invoking human's session.
    pub(crate) fn operator(&self) -> &Session {
        &self.operator
    }

    /// What is being imported.
    pub(crate) fn source_label(&self) -> &str {
        &self.source_label
    }
}

/// An import run in progress inside one plugin invocation.
#[derive(Debug)]
pub(crate) struct ActiveRun {
    pending: Arc<PendingRun>,
    counts: ImportCounts,
    resolved: Vec<ResolvedItem>,
    cancelled: bool,
}

impl ActiveRun {
    /// A run over `dataset`, described by `template`, whose document declared `declared` (its header's
    /// fingerprint and export date).
    pub(crate) fn new(
        template: RunTemplate,
        dataset: ChosenDataset,
        (dataset_hint, file_asserted_at): (Option<String>, Option<Timestamp>),
    ) -> Self {
        let RunTemplate {
            operator,
            source_label,
            plugin,
            plugin_version,
        } = template;
        let run = NewImportRun {
            plugin,
            plugin_version,
            dataset: dataset.id,
            dataset_label: dataset.label,
            source_label,
            file_asserted_at: None,
            dataset_hint,
        };
        let pending = Arc::new(PendingRun::new(operator, run));
        pending.set_file_asserted_at(file_asserted_at);
        Self {
            pending,
            counts: ImportCounts::default(),
            resolved: Vec::new(),
            cancelled: false,
        }
    }

    /// The pending run the plugin's session writes as part of.
    pub(crate) fn pending(&self) -> &Arc<PendingRun> {
        &self.pending
    }

    /// Adds what one commit wrote to the run's counts and resolutions.
    pub(crate) fn absorb(&mut self, outcome: &CommitOutcome) {
        for (kind, count) in &outcome.created {
            let total = self.counts.created.entry(kind.clone()).or_insert(0);
            *total = total.saturating_add(*count);
        }
        let resolved = u32::try_from(outcome.resolved.len()).unwrap_or(u32::MAX);
        self.counts.resolved = self.counts.resolved.saturating_add(resolved);
        self.resolved.extend(outcome.resolved.iter().cloned());
    }

    /// Notes that the operator asked the import to stop.
    pub(crate) fn cancel(&mut self) {
        self.cancelled = true;
    }
}

/// How an import invocation ended, for closing its run.
pub(crate) enum Ending<'a> {
    /// A bulk import returned its record count.
    Bulk(&'a Result<u32, PluginError>),
    /// An assisted session returned its summary. Its cancel only stops a fetch within the session, so
    /// a returned session always finished.
    Assisted(&'a Result<String, PluginError>),
}

impl Ending<'_> {
    /// Whether the invocation returned normally.
    pub(crate) fn succeeded(&self) -> bool {
        match self {
            Self::Bulk(result) => result.is_ok(),
            Self::Assisted(result) => result.is_ok(),
        }
    }
}

/// Closes `run`, if it started: finished when the import returned normally, abandoned when it failed
/// or a bulk import was cancelled.
///
/// # Errors
///
/// An [`AppError`](vitni_app::AppError) when the closing event cannot be written.
pub(crate) async fn close(
    workspace: &Workspace,
    run: ActiveRun,
    ending: &Ending<'_>,
) -> Result<(), vitni_app::AppError> {
    let ActiveRun {
        pending,
        mut counts,
        resolved,
        cancelled,
        ..
    } = run;
    if !pending.started() {
        if resolved.is_empty() {
            return Ok(());
        }
        pending.ensure_started(workspace.store()).await?;
    }
    counts.commands = pending.commands();
    let reason = match ending {
        Ending::Bulk(Ok(records)) => {
            counts.records = Some(*records);
            cancelled.then_some(AbandonReason::Cancelled)
        }
        Ending::Assisted(Ok(_)) => None,
        Ending::Bulk(Err(error)) | Ending::Assisted(Err(error)) => Some(abandon_reason(error)),
    };
    let (operator, id) = (pending.operator(), pending.id());
    match reason {
        None => vitni_app::finish_import_run(workspace, operator, id, resolved, counts).await,
        Some(reason) => vitni_app::abandon_import_run(workspace, operator, id, resolved, counts, reason).await,
    }
}

fn abandon_reason(error: &PluginError) -> AbandonReason {
    match error {
        PluginError::Guest(message) => AbandonReason::GuestError {
            message: message.clone(),
        },
        PluginError::ResourceLimit(_) => AbandonReason::ResourceLimit,
        PluginError::Runtime(message) | PluginError::Signature(message) => AbandonReason::Runtime {
            message: message.clone(),
        },
        PluginError::Commit(message) => AbandonReason::Commit {
            message: message.clone(),
        },
        PluginError::Dataset(error) => AbandonReason::Runtime {
            message: error.to_string(),
        },
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;
    use vitni_app::{
        AgentKind, AppDefaults, ChosenDataset, CommitOutcome, DatasetId, OperatorConfig, ResolutionDecision,
        ResolvedItem, Session, Workspace, WorkspaceDefaults, list_import_runs,
    };
    use vitni_core::ids::AgentId;
    use vitni_core::provenance::Agent;

    use super::{ActiveRun, Ending, ImportRunSpec, RunDataset, close};
    use crate::DeferMatches;

    fn operator() -> OperatorConfig {
        OperatorConfig {
            id: AgentId::from_uuid(Uuid::from_u128(1)),
            display: Some("Tester".to_owned()),
            email: None,
        }
    }

    fn spec() -> ImportRunSpec {
        ImportRunSpec {
            operator: Session::new(Agent {
                kind: AgentKind::Human,
                id: AgentId::from_uuid(Uuid::from_u128(1)),
                display: Some("Tester".to_owned()),
            }),
            dataset: RunDataset::Chosen(chosen()),
            source_label: "tree.ged".to_owned(),
            plugin: "gedcom-import".to_owned(),
            plugin_version: "0.1.0".to_owned(),
            reviewer: Box::new(DeferMatches),
        }
    }

    fn chosen() -> ChosenDataset {
        ChosenDataset {
            id: DatasetId::lineage("gedcom", Uuid::from_u128(5)),
            label: "tree.ged".to_owned(),
        }
    }

    async fn workspace(dir: &std::path::Path) -> Workspace {
        let root = dir.join("ws");
        Workspace::init(&root, &operator(), &AppDefaults::default(), None).expect("init");
        Workspace::open(&root, &operator(), &WorkspaceDefaults::default())
            .await
            .expect("open")
    }

    #[tokio::test]
    async fn a_run_that_wrote_nothing_is_not_recorded() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let run = ActiveRun::new(spec().into_parts().1, chosen(), (None, None));
        close(&workspace, run, &Ending::Bulk(&Ok(3))).await.expect("closed");
        let checked = list_import_runs(&workspace).await.expect("runs");
        assert!(checked.is_empty(), "{checked:?}");
    }

    #[tokio::test]
    async fn a_run_that_only_resolved_items_is_recorded_with_them() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let mut run = ActiveRun::new(spec().into_parts().1, chosen(), (None, None));
        run.absorb(&CommitOutcome {
            resolved: vec![ResolvedItem {
                record: "I1".to_owned(),
                item: None,
                kind: "person".to_owned(),
                aggregate_id: Uuid::from_u128(9),
                decision: ResolutionDecision::ExternalId,
            }],
            ..CommitOutcome::default()
        });
        let id = run.pending().id();
        close(&workspace, run, &Ending::Bulk(&Ok(1))).await.expect("closed");
        let runs = list_import_runs(&workspace).await.expect("runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, id);
        assert_eq!(runs[0].counts.resolved, 1);
    }
}
