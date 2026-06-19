//! Citation commands — imperative operator intent (data-model §10).

use crate::ids::{CitationId, HumanId, SourceId};
use crate::provenance::AssertionMeta;

/// Operator intent against a Citation aggregate (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CitationCommand {
    /// Create a new citation against a source.
    CreateCitation {
        /// The application-generated id for the new citation.
        citation_id: CitationId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// The source this citation points into (the cross-aggregate reference).
        source_id: SourceId,
    },
    /// Set (or change) the citation's page / locator within the source.
    SetPage {
        /// The target citation.
        citation_id: CitationId,
        /// The page / locator text (e.g. `p. 42`, `entry 17`).
        page: String,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the Citation aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: CitationCommand,
}
