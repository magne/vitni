//! The import run an import invocation writes (ADR 0037 §5): who started it, over which dataset, and
//! what the run has written so far.
//!
//! The run is started lazily, by the first write the origin gate lets through (`vitni_app`'s
//! [`PendingRun`]), so an invocation that writes nothing (an assisted session the operator closes at
//! once, an importer that fails while parsing, a re-import of a file already on record) leaves no run
//! behind. It is closed once the guest returns, finished or abandoned.

use std::sync::Arc;

use vitni_app::{
    AbandonReason, CommitOutcome, DatasetId, ImportCounts, NewImportRun, PendingRun, ResolvedItem, Session, Workspace,
};

use crate::error::PluginError;

/// What an import invocation's run is over, supplied by the frontend that starts the import.
#[derive(Debug, Clone)]
pub struct ImportRunSpec {
    /// The invoking human's session: the run's operator, while every imported assertion keeps the
    /// importer's `Software` agent (ADR 0037 §5).
    pub operator: Session,
    /// The dataset the run's records belong to.
    pub dataset: DatasetId,
    /// The dataset's label.
    pub dataset_label: String,
    /// What is being imported: a file name, or an assisted session's request.
    pub source_label: String,
    /// The importing plugin's id.
    pub plugin: String,
    /// The importing plugin's own version, from its manifest.
    pub plugin_version: String,
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
    pub(crate) fn new(spec: ImportRunSpec) -> Self {
        let ImportRunSpec {
            operator,
            dataset,
            dataset_label,
            source_label,
            plugin,
            plugin_version,
        } = spec;
        let run = NewImportRun {
            plugin,
            plugin_version,
            dataset,
            dataset_label,
            source_label,
            file_asserted_at: None,
        };
        let pending = Arc::new(PendingRun::new(operator, run));
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
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;
    use vitni_app::{
        AgentKind, AppDefaults, CommitOutcome, DatasetId, OperatorConfig, ResolutionDecision, ResolvedItem, Session,
        Workspace, WorkspaceDefaults, list_import_runs,
    };
    use vitni_core::ids::AgentId;
    use vitni_core::provenance::Agent;

    use super::{ActiveRun, Ending, ImportRunSpec, close};

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
            dataset: DatasetId::lineage("gedcom", Uuid::from_u128(5)),
            dataset_label: "tree.ged".to_owned(),
            source_label: "tree.ged".to_owned(),
            plugin: "gedcom-import".to_owned(),
            plugin_version: "0.1.0".to_owned(),
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
        let run = ActiveRun::new(spec());
        close(&workspace, run, &Ending::Bulk(&Ok(3))).await.expect("closed");
        let checked = list_import_runs(&workspace).await.expect("runs");
        assert!(checked.is_empty(), "{checked:?}");
    }

    #[tokio::test]
    async fn a_run_that_only_resolved_items_is_recorded_with_them() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let mut run = ActiveRun::new(spec());
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
