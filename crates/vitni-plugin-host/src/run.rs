//! The import run an import invocation writes (ADR 0037 §5): who started it, over which dataset,
//! the origin the guest last declared, and what the run has written so far.
//!
//! The run is started lazily, on the guest's first write, so an invocation that writes nothing (an
//! assisted session the operator closes at once, an importer that fails while parsing) leaves no run
//! behind. It is closed once the guest returns, finished or abandoned.

use vitni_app::{
    AbandonReason, DatasetId, ImportCounts, ImportRunId, NewImportRun, RecordOrigin, ResolutionDecision, ResolvedItem,
    Session, Workspace,
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

/// The source record the guest is writing from, as it last declared with `set-origin`.
#[derive(Debug, Clone)]
struct OriginKey {
    record: String,
    item: Option<String>,
}

/// An import run in progress inside one plugin invocation.
#[derive(Debug)]
pub(crate) struct ActiveRun {
    spec: ImportRunSpec,
    id: Option<ImportRunId>,
    origin: Option<OriginKey>,
    counts: ImportCounts,
    resolved: Vec<ResolvedItem>,
    cancelled: bool,
}

/// Why a write was refused before it reached the workspace.
#[derive(Debug)]
pub(crate) enum WriteRefusal {
    /// The guest has not declared the record it is writing from.
    NoOrigin,
    /// The run could not be started.
    Start(vitni_app::AppError),
}

impl ActiveRun {
    pub(crate) fn new(spec: ImportRunSpec) -> Self {
        Self {
            spec,
            id: None,
            origin: None,
            counts: ImportCounts::default(),
            resolved: Vec::new(),
            cancelled: false,
        }
    }

    /// Sets (or, with `None`, clears) the record later writes are stamped with.
    pub(crate) fn set_origin(&mut self, origin: Option<(String, Option<String>)>) {
        self.origin = origin.map(|(record, item)| OriginKey { record, item });
    }

    /// Admits one write: refuses it while no origin is set, starts the run on the first one, and
    /// counts it.
    pub(crate) async fn admit_write(
        &mut self,
        workspace: &Workspace,
        file_asserted_at: Option<vitni_app::Timestamp>,
    ) -> Result<(), WriteRefusal> {
        if self.origin.is_none() {
            return Err(WriteRefusal::NoOrigin);
        }
        if self.id.is_none() {
            let run = NewImportRun {
                plugin: self.spec.plugin.clone(),
                plugin_version: self.spec.plugin_version.clone(),
                dataset: self.spec.dataset.clone(),
                dataset_label: self.spec.dataset_label.clone(),
                source_label: self.spec.source_label.clone(),
                file_asserted_at,
            };
            let id = vitni_app::start_import_run(workspace, &self.spec.operator, run)
                .await
                .map_err(WriteRefusal::Start)?;
            self.id = Some(id);
        }
        self.counts.commands = self.counts.commands.saturating_add(1);
        Ok(())
    }

    /// The origin to stamp on the write being made, once the run has started.
    pub(crate) fn origin(&self) -> Option<RecordOrigin> {
        let (Some(run), Some(key)) = (self.id, &self.origin) else {
            return None;
        };
        Some(RecordOrigin {
            dataset: self.spec.dataset.clone(),
            record: key.record.clone(),
            item: key.item.clone(),
            digest: None,
            run,
        })
    }

    /// Counts one aggregate of `kind` (its `Aggregate::TYPE`) the run created.
    pub(crate) fn created(&mut self, kind: &str) {
        let count = self.counts.created.entry(kind.to_owned()).or_insert(0);
        *count = count.saturating_add(1);
    }

    /// Records that the current item resolved onto the existing aggregate `aggregate_id` of `kind`.
    pub(crate) fn resolved(&mut self, kind: &str, aggregate_id: uuid::Uuid) {
        let Some(key) = &self.origin else {
            return;
        };
        self.resolved.push(ResolvedItem {
            record: key.record.clone(),
            item: key.item.clone(),
            kind: kind.to_owned(),
            aggregate_id,
            decision: ResolutionDecision::ExternalId,
        });
        self.counts.resolved = self.counts.resolved.saturating_add(1);
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
        spec,
        id,
        mut counts,
        resolved,
        cancelled,
        ..
    } = run;
    let Some(id) = id else {
        return Ok(());
    };
    let reason = match ending {
        Ending::Bulk(Ok(records)) => {
            counts.records = Some(*records);
            cancelled.then_some(AbandonReason::Cancelled)
        }
        Ending::Assisted(Ok(_)) => None,
        Ending::Bulk(Err(error)) | Ending::Assisted(Err(error)) => Some(abandon_reason(error)),
    };
    match reason {
        None => vitni_app::finish_import_run(workspace, &spec.operator, id, resolved, counts).await,
        Some(reason) => vitni_app::abandon_import_run(workspace, &spec.operator, id, resolved, counts, reason).await,
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
    }
}

#[cfg(test)]
mod tests {
    use uuid::Uuid;
    use vitni_app::{
        AgentKind, AppDefaults, DatasetId, OperatorConfig, Session, Workspace, WorkspaceDefaults, list_import_runs,
    };
    use vitni_core::ids::AgentId;
    use vitni_core::provenance::Agent;

    use super::{ActiveRun, ImportRunSpec, WriteRefusal};

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
    async fn a_write_before_any_origin_is_refused_and_starts_no_run() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let mut run = ActiveRun::new(spec());
        let refused = run.admit_write(&workspace, None).await;
        assert!(matches!(refused, Err(WriteRefusal::NoOrigin)), "{refused:?}");
        assert!(list_import_runs(&workspace).await.expect("runs").is_empty());
    }

    #[tokio::test]
    async fn the_first_admitted_write_starts_the_run_and_stamps_its_origin() {
        let dir = tempfile::tempdir().expect("tempdir");
        let workspace = workspace(dir.path()).await;
        let mut run = ActiveRun::new(spec());
        run.set_origin(Some(("I1".to_owned(), Some("event:BIRT:0".to_owned()))));
        assert_eq!(run.origin(), None, "nothing to stamp before the run starts");
        run.admit_write(&workspace, None).await.expect("admitted");
        let origin = run.origin().expect("stamped");
        assert_eq!(
            (origin.record.as_str(), origin.item.as_deref()),
            ("I1", Some("event:BIRT:0"))
        );
        let runs = list_import_runs(&workspace).await.expect("runs");
        assert_eq!(runs.len(), 1);
        assert_eq!(runs[0].id, origin.run);
    }
}
