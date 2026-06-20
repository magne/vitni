//! Citation events — the past-tense assertions the aggregate produces (data-model §10).

use serde::{Deserialize, Serialize};

use crate::assertions::{Envelope, EventBody};
use crate::ids::{AssertionId, CitationId, HumanId, SourceId};

/// A single Citation assertion plus its provenance envelope (ADR 0004 §1).
pub type CitationEvent = Envelope<CitationEventBody>;

/// The Citation claim variants (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum CitationEventBody {
    /// A citation aggregate was created, pointing at a source.
    CitationCreated {
        /// The created citation.
        citation_id: CitationId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// The source this citation points into.
        source_id: SourceId,
    },
    /// The citation's page / locator was set / changed.
    PageSet {
        /// The citation.
        citation_id: CitationId,
        /// The page / locator text.
        page: String,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The citation.
        citation_id: CitationId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The citation.
        citation_id: CitationId,
        /// The assertion being superseded.
        target: AssertionId,
    },
}

impl EventBody for CitationEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::CitationCreated { .. } => "CitationCreated",
            Self::PageSet { .. } => "PageSet",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
        }
    }

    fn version(&self) -> &'static str {
        // Per-variant; bumped only on an additive payload change (ADR 0004 §4).
        "1.0"
    }
}
