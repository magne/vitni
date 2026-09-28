//! `ImportRun` events (ADR 0037 §5).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use uuid::Uuid;

use crate::assertions::{Envelope, EventBody};
use crate::ids::ImportRunId;
use crate::origin::DatasetId;
use crate::provenance::Timestamp;

/// A single `ImportRun` event plus its provenance envelope (ADR 0004 §1).
pub type ImportRunEvent = Envelope<ImportRunEventBody>;

/// How an incoming item was resolved onto an existing aggregate without creating one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum ResolutionDecision {
    /// Deterministic identity: the item's `ExternalId` already named an aggregate (ADR 0013 §6).
    ExternalId,
}

/// One incoming item the run resolved onto an existing aggregate.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ResolvedItem {
    /// The record's id within the run's dataset.
    pub record: String,
    /// The entity within the record, when the record yields more than one.
    pub item: Option<String>,
    /// The resolved aggregate's kind (its `Aggregate::TYPE`, e.g. `person`).
    pub kind: String,
    /// The resolved aggregate's id.
    pub aggregate_id: Uuid,
    /// How it was resolved.
    pub decision: ResolutionDecision,
}

/// What a run wrote. Every field defaults, so a later counter decodes from an older run.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct ImportCounts {
    /// Aggregates created, by kind (`Aggregate::TYPE`).
    #[serde(default)]
    pub created: BTreeMap<String, u32>,
    /// Incoming items resolved onto existing aggregates.
    #[serde(default)]
    pub resolved: u32,
    /// Write commands the importer issued.
    #[serde(default)]
    pub commands: u32,
    /// The importer's own record count, when it reports one (a bulk import's return value).
    #[serde(default)]
    pub records: Option<u32>,
}

impl ImportCounts {
    /// The number of aggregates the run created, over every kind.
    #[must_use]
    pub fn created_total(&self) -> u32 {
        let mut total = 0_u32;
        for count in self.created.values() {
            total = total.saturating_add(*count);
        }
        total
    }
}

/// Why a run ended early.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum AbandonReason {
    /// The operator cancelled it.
    Cancelled,
    /// The importer reported a failure.
    GuestError {
        /// The importer's message.
        message: String,
    },
    /// The importer exhausted its fuel or memory budget.
    ResourceLimit,
    /// The importer trapped or could not be run.
    Runtime {
        /// The runtime's message.
        message: String,
    },
}

/// The `ImportRun` event variants.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, strum::VariantNames)]
#[serde(tag = "type")]
pub enum ImportRunEventBody {
    /// A run started.
    ImportRunStarted {
        /// The run.
        run_id: ImportRunId,
        /// The importing plugin's id.
        plugin: String,
        /// The importing plugin's version.
        plugin_version: String,
        /// The dataset the run's records belong to.
        dataset: DatasetId,
        /// The dataset's display label.
        dataset_label: String,
        /// What was imported.
        source_label: String,
        /// The document's own export date, if it carries one.
        file_asserted_at: Option<Timestamp>,
    },
    /// An incoming item was resolved onto an existing aggregate, creating nothing.
    ItemResolved {
        /// The run.
        run_id: ImportRunId,
        /// The run's dataset.
        dataset: DatasetId,
        /// The record's id within the dataset.
        record: String,
        /// The entity within the record.
        item: Option<String>,
        /// The resolved aggregate's kind.
        kind: String,
        /// The resolved aggregate's id.
        aggregate_id: Uuid,
        /// How it was resolved.
        decision: ResolutionDecision,
    },
    /// A run ended normally.
    ImportRunFinished {
        /// The run.
        run_id: ImportRunId,
        /// What it wrote.
        counts: ImportCounts,
    },
    /// A run ended early.
    ImportRunAbandoned {
        /// The run.
        run_id: ImportRunId,
        /// Why it ended.
        reason: AbandonReason,
        /// What it wrote before it ended.
        counts: ImportCounts,
    },
}

impl EventBody for ImportRunEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::ImportRunStarted { .. } => "ImportRunStarted",
            Self::ItemResolved { .. } => "ItemResolved",
            Self::ImportRunFinished { .. } => "ImportRunFinished",
            Self::ImportRunAbandoned { .. } => "ImportRunAbandoned",
        }
    }

    fn version(&self) -> &'static str {
        "1.0"
    }
}
