//! Source events — the past-tense assertions the aggregate produces (data-model §10).

use cqrs_es::DomainEvent;
use serde::{Deserialize, Serialize};

use crate::ids::{AssertionId, HumanId, SourceId};
use crate::provenance::{AssertionMeta, EventContext};

/// A single Source assertion plus its provenance envelope (ADR 0004 §1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceEvent {
    /// Identity of this assertion, so a correction can target it (ADR 0004 §2).
    pub assertion_id: AssertionId,
    /// Who / when / why / how sure / on what evidence (data-model §8).
    pub context: EventContext,
    /// The claim itself.
    #[serde(flatten)]
    pub body: SourceEventBody,
}

impl SourceEvent {
    /// Stamps `body` with the supplied assertion id and context (ADR 0004 §3).
    #[must_use]
    pub fn new(meta: &AssertionMeta, body: SourceEventBody) -> Self {
        Self {
            assertion_id: meta.assertion_id,
            context: meta.context.clone(),
            body,
        }
    }
}

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
}

impl SourceEventBody {
    /// The variant name, used as the `cqrs-es` event type (ADR 0004 §4).
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::SourceCreated { .. } => "SourceCreated",
            Self::TitleSet { .. } => "TitleSet",
        }
    }
}

impl DomainEvent for SourceEvent {
    fn event_type(&self) -> String {
        self.body.type_name().to_owned()
    }

    fn event_version(&self) -> String {
        // Bumped only on an incompatible payload change (ADR 0004 §4).
        "1.0".to_owned()
    }
}
