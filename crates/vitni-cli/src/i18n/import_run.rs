use super::{
    AbandonReason, DatasetError, DatasetSummary, ImportRunError, ImportRunStatus, ImportRunSummary, Localizer, fl,
};

impl Localizer {
    /// `No import runs yet.`
    #[must_use]
    pub fn import_run_list_empty(&self) -> String {
        fl!(self.loader, "import-run-list-empty")
    }

    /// One run line: id, start, status, importer, source, dataset, operator and counts.
    #[must_use]
    pub fn import_run_line(&self, run: &ImportRunSummary) -> String {
        let operator = match &run.operator_display {
            Some(name) => name.clone(),
            None => fl!(self.loader, "no-value"),
        };
        fl!(
            self.loader,
            "import-run-summary",
            id = run.id.to_string(),
            started = run.started_at.into_inner().date().to_string(),
            status = self.import_run_status(&run.status),
            plugin = run.plugin.clone(),
            version = run.plugin_version.clone(),
            source = run.source_label.clone(),
            dataset = run.dataset.to_string(),
            operator = operator,
            created = run.counts.created_total(),
            resolved = run.counts.resolved
        )
    }

    fn import_run_status(&self, status: &ImportRunStatus) -> String {
        match status {
            ImportRunStatus::NotStarted | ImportRunStatus::Running => fl!(self.loader, "import-run-status-running"),
            ImportRunStatus::Finished => fl!(self.loader, "import-run-status-finished"),
            ImportRunStatus::Abandoned { reason } => match reason {
                AbandonReason::Cancelled => fl!(self.loader, "import-run-status-cancelled"),
                AbandonReason::GuestError { .. }
                | AbandonReason::ResourceLimit
                | AbandonReason::Runtime { .. }
                | AbandonReason::Commit { .. } => {
                    fl!(self.loader, "import-run-status-failed")
                }
            },
        }
    }

    /// `No datasets yet.`
    #[must_use]
    pub fn dataset_list_empty(&self) -> String {
        fl!(self.loader, "dataset-list-empty")
    }

    /// One dataset line: id, label and run count.
    #[must_use]
    pub fn dataset_line(&self, dataset: &DatasetSummary) -> String {
        fl!(
            self.loader,
            "dataset-summary",
            id = dataset.id.to_string(),
            label = dataset.label.clone(),
            runs = dataset.runs
        )
    }

    /// The question whether a file goes into the dataset the host proposed for it (ADR 0037 §3).
    #[must_use]
    pub fn import_dataset_proposed(&self, label: &str, shared: usize, keys: usize) -> String {
        fl!(
            self.loader,
            "import-dataset-proposed",
            label = label,
            shared = shared,
            keys = keys
        )
    }

    /// The notice that `--yes` accepted the dataset the host proposed for a file.
    #[must_use]
    pub fn import_dataset_proposed_accepted(&self, label: &str, shared: usize, keys: usize) -> String {
        fl!(
            self.loader,
            "import-dataset-proposed-accepted",
            label = label,
            shared = shared,
            keys = keys
        )
    }

    pub(super) fn import_run_error(&self, error: &ImportRunError) -> String {
        match error {
            ImportRunError::NotFound(id) => fl!(self.loader, "err-import-run-not-exist", id = id.to_string()),
            ImportRunError::AlreadyExists(id) => fl!(self.loader, "err-import-run-exists", id = id.to_string()),
            ImportRunError::AlreadyEnded(id) => fl!(self.loader, "err-import-run-ended", id = id.to_string()),
        }
    }

    pub(super) fn dataset_error(&self, error: &DatasetError) -> String {
        match error {
            DatasetError::NotFound { scheme, query } => fl!(
                self.loader,
                "err-dataset-not-found",
                scheme = scheme.clone(),
                query = query.clone()
            ),
            DatasetError::Ambiguous { query, candidates } => fl!(
                self.loader,
                "err-dataset-ambiguous",
                query = query.clone(),
                candidates = candidates.join(", ")
            ),
            DatasetError::Required { scheme, candidates } => fl!(
                self.loader,
                "err-dataset-required",
                scheme = scheme.clone(),
                candidates = candidates.join(", ")
            ),
            DatasetError::Global { scheme } => fl!(self.loader, "err-dataset-global", scheme = scheme.clone()),
        }
    }
}
