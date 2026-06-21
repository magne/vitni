//! Source events — the past-tense assertions the aggregate produces (data-model §10).

use serde::{Deserialize, Serialize};

use crate::assertions::{Envelope, EventBody};
use crate::ids::{AssertionId, HumanId, SourceId};

/// A single Source assertion plus its provenance envelope (ADR 0004 §1).
pub type SourceEvent = Envelope<SourceEventBody>;

/// The Source claim variants (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum SourceEventBody {
    /// A source aggregate was created.
    SourceCreated {
        /// The created source.
        source_id: SourceId,
        /// The user-facing identifier.
        human_id: HumanId,
    },
    /// The source's title was set / changed.
    TitleSet {
        /// The source.
        source_id: SourceId,
        /// The bibliographic title.
        title: String,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The source.
        source_id: SourceId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The source.
        source_id: SourceId,
        /// The assertion being superseded.
        target: AssertionId,
    },
}

impl EventBody for SourceEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::SourceCreated { .. } => "SourceCreated",
            Self::TitleSet { .. } => "TitleSet",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
        }
    }

    fn version(&self) -> &'static str {
        // Per-variant; bumped only on an additive payload change (ADR 0004 §4).
        "1.0"
    }
}
