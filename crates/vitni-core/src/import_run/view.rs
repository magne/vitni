//! [`ImportRunView`] — the read model for an import run, and the source of the datasets projection.
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009). A run has no `HumanId`; it is
//! looked up by its aggregate id.

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use crate::ids::ImportRunId;
use crate::import_run::decide::evolve;
use crate::import_run::event::ImportCounts;
use crate::import_run::state::{ImportRunState, ImportRunStatus};
use crate::origin::DatasetId;
use crate::provenance::{Agent, Timestamp};

/// One import run, derived from the event log.
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportRunView {
    state: ImportRunState,
}

impl ImportRunView {
    /// The run's id, once started.
    #[must_use]
    pub fn run_id(&self) -> Option<ImportRunId> {
        self.state.run_id
    }

    /// Where the run is in its life.
    #[must_use]
    pub fn status(&self) -> &ImportRunStatus {
        &self.state.status
    }

    /// Who started the run.
    #[must_use]
    pub fn operator(&self) -> Option<&Agent> {
        self.state.operator.as_ref()
    }

    /// When the run started.
    #[must_use]
    pub fn started_at(&self) -> Option<Timestamp> {
        self.state.started_at
    }

    /// When the run ended, once it has.
    #[must_use]
    pub fn ended_at(&self) -> Option<Timestamp> {
        self.state.ended_at
    }

    /// The importing plugin's id.
    #[must_use]
    pub fn plugin(&self) -> &str {
        &self.state.plugin
    }

    /// The importing plugin's version.
    #[must_use]
    pub fn plugin_version(&self) -> &str {
        &self.state.plugin_version
    }

    /// The dataset the run's records belong to, once started.
    #[must_use]
    pub fn dataset(&self) -> Option<&DatasetId> {
        self.state.dataset.as_ref()
    }

    /// The dataset's display label.
    #[must_use]
    pub fn dataset_label(&self) -> &str {
        &self.state.dataset_label
    }

    /// What was imported.
    #[must_use]
    pub fn source_label(&self) -> &str {
        &self.state.source_label
    }

    /// Where a bulk import read its file from, if the run recorded it (ADR 0040 §5).
    #[must_use]
    pub fn source_path(&self) -> Option<&str> {
        self.state.source_path.as_deref()
    }

    /// The document's own export date, if it carries one.
    #[must_use]
    pub fn file_asserted_at(&self) -> Option<Timestamp> {
        self.state.file_asserted_at
    }

    /// The document header's fingerprint, if the importer declared one (ADR 0037 §3).
    #[must_use]
    pub fn dataset_hint(&self) -> Option<&str> {
        self.state.dataset_hint.as_deref()
    }

    /// What the run wrote (zero until it has ended).
    #[must_use]
    pub fn counts(&self) -> &ImportCounts {
        &self.state.counts
    }
}

impl View<ImportRunState> for ImportRunView {
    fn update(&mut self, event: &EventEnvelope<ImportRunState>) {
        evolve(&mut self.state, &event.payload);
    }
}
