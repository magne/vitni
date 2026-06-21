//! Source commands — imperative operator intent (data-model §10).

use crate::ids::{AssertionId, HumanId, SourceId};
use crate::provenance::AssertionMeta;

/// Operator intent against a Source aggregate (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SourceCommand {
    /// Create a new source.
    CreateSource {
        /// The application-generated id for the new source.
        source_id: SourceId,
        /// The user-facing identifier.
        human_id: HumanId,
    },
    /// Set (or change) the source's title.
    SetTitle {
        /// The target source.
        source_id: SourceId,
        /// The bibliographic title.
        title: String,
    },
    /// Retract a prior assertion (non-destructive).
    RetractAssertion {
        /// The target source.
        source_id: SourceId,
        /// The assertion to retract.
        target: AssertionId,
    },
    /// Supersede a prior assertion with a replacement command.
    SupersedeAssertion {
        /// The target source.
        source_id: SourceId,
        /// The assertion to supersede.
        target: AssertionId,
        /// The command producing the replacement assertion.
        replacement: Box<SourceCommand>,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the Source aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: SourceCommand,
}
