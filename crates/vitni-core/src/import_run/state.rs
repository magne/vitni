//! [`ImportRunState`] — the folded aggregate state used by the decision core.

use serde::{Deserialize, Serialize};

use crate::ids::ImportRunId;
use crate::import_run::event::{AbandonReason, ImportCounts};
use crate::origin::DatasetId;
use crate::provenance::{Agent, Timestamp};

/// Where a run is in its life.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum ImportRunStatus {
    /// Not started (the state before `ImportRunStarted`).
    #[default]
    NotStarted,
    /// Started and not yet ended.
    Running,
    /// Ended normally.
    Finished,
    /// Ended early.
    Abandoned {
        /// Why it ended.
        reason: AbandonReason,
    },
}

/// The folded state of an `ImportRun` aggregate.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportRunState {
    /// Where the run is in its life.
    pub status: ImportRunStatus,
    /// The run's id (set on start).
    pub run_id: Option<ImportRunId>,
    /// Who started the run (the invoking human).
    pub operator: Option<Agent>,
    /// When the run started.
    pub started_at: Option<Timestamp>,
    /// When the run ended.
    pub ended_at: Option<Timestamp>,
    /// The importing plugin's id.
    pub plugin: String,
    /// The importing plugin's version.
    pub plugin_version: String,
    /// The dataset the run's records belong to.
    pub dataset: Option<DatasetId>,
    /// The dataset's display label.
    pub dataset_label: String,
    /// What was imported.
    pub source_label: String,
    /// Where a bulk import read its file from, if the run recorded it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_path: Option<String>,
    /// The document's own export date, if it carries one.
    pub file_asserted_at: Option<Timestamp>,
    /// The document header's fingerprint, if the importer declared one.
    pub dataset_hint: Option<String>,
    /// What the run wrote, once it has ended.
    pub counts: ImportCounts,
}
