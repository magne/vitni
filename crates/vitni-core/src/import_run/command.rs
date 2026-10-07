//! `ImportRun` commands (ADR 0037 §5).

use crate::ids::ImportRunId;
use crate::import_run::event::{AbandonReason, ImportCounts, ResolvedItem};
use crate::origin::DatasetId;
use crate::provenance::{AssertionMeta, Timestamp};

/// What an import run is started over: the importer, the dataset, and the document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewImportRun {
    /// The importing plugin's id (e.g. `gedcom-import`).
    pub plugin: String,
    /// The importing plugin's version, from its manifest.
    pub plugin_version: String,
    /// The dataset the run's records belong to.
    pub dataset: DatasetId,
    /// The dataset's display label (the source file name for a new lineage).
    pub dataset_label: String,
    /// What was imported: a file name, or an assisted session's request.
    pub source_label: String,
    /// Where a bulk import read its file from, as an absolute path; `None` for an assisted session.
    /// Resuming an abandoned run re-reads it (ADR 0040 §5).
    pub source_path: Option<String>,
    /// The document's own export date, when its header carries one (ADR 0029 §2).
    pub file_asserted_at: Option<Timestamp>,
    /// The document header's fingerprint, when the importer declared one: compared with later
    /// imports' to propose their dataset (ADR 0037 §3).
    pub dataset_hint: Option<String>,
}

/// Operator intent against an `ImportRun` aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ImportRunCommand {
    /// Start a run.
    StartImportRun {
        /// The application-generated id for the new run.
        run_id: ImportRunId,
        /// What the run is over.
        run: NewImportRun,
    },
    /// End a run that completed, recording the resolutions it made that created nothing.
    FinishImportRun {
        /// The target run.
        run_id: ImportRunId,
        /// The incoming items resolved onto existing aggregates.
        resolved: Vec<ResolvedItem>,
        /// What the run wrote.
        counts: ImportCounts,
    },
    /// End a run that did not complete: cancelled, or failed.
    AbandonImportRun {
        /// The target run.
        run_id: ImportRunId,
        /// The incoming items resolved onto existing aggregates before it ended.
        resolved: Vec<ResolvedItem>,
        /// What the run wrote before it ended.
        counts: ImportCounts,
        /// Why it ended.
        reason: AbandonReason,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ImportRunCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: ImportRunCommand,
}
