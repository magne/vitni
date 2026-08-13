//! Citation events — the past-tense assertions the aggregate produces (data-model §10).

use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::{Envelope, EventBody};
use crate::date::GenealogicalDate;
use crate::enums::Restriction;
use crate::ids::{AssertionId, CitationId, HumanId, NoteId, SourceId, TagId};
use crate::provenance::{Confidence, EvidenceAnalysis};
use crate::text::{Attribute, MediaRef};

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
    /// The date of the cited record was asserted.
    DateAsserted {
        /// The citation.
        citation_id: CitationId,
        /// The date.
        date: GenealogicalDate,
    },
    /// The operator's confidence in the citation was set / changed.
    ConfidenceSet {
        /// The citation.
        citation_id: CitationId,
        /// The confidence level.
        confidence: Confidence,
    },
    /// The citation's evidence analysis was set / changed.
    EvidenceAnalysisSet {
        /// The citation.
        citation_id: CitationId,
        /// The evidence analysis.
        analysis: EvidenceAnalysis,
    },
    /// A typed attribute was added to the citation.
    AttributeAdded {
        /// The citation.
        citation_id: CitationId,
        /// The attribute.
        attribute: Attribute,
    },
    /// A media reference was attached to the citation.
    MediaAttached {
        /// The citation.
        citation_id: CitationId,
        /// The media reference.
        media: MediaRef,
    },
    /// A note was attached to the citation.
    NoteAttached {
        /// The citation.
        citation_id: CitationId,
        /// The attached note.
        note_id: NoteId,
    },
    /// A tag was applied to the citation.
    Tagged {
        /// The citation.
        citation_id: CitationId,
        /// The applied tag.
        tag_id: TagId,
    },
    /// A tag was removed from the citation.
    Untagged {
        /// The citation.
        citation_id: CitationId,
        /// The removed tag.
        tag_id: TagId,
    },
    /// The citation's privacy restrictions were set / changed (GEDCOM `RESN` — data-model §6).
    RestrictionsChanged {
        /// The citation.
        citation_id: CitationId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
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
    /// The citation's user-facing identifier was changed (data-model §7).
    HumanIdChanged {
        /// The citation.
        citation_id: CitationId,
        /// The new user-facing identifier.
        human_id: HumanId,
        /// The identifier in effect before this change (for the audit trail).
        old_human_id: HumanId,
    },
}

impl EventBody for CitationEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::CitationCreated { .. } => "CitationCreated",
            Self::PageSet { .. } => "PageSet",
            Self::DateAsserted { .. } => "DateAsserted",
            Self::ConfidenceSet { .. } => "ConfidenceSet",
            Self::EvidenceAnalysisSet { .. } => "EvidenceAnalysisSet",
            Self::AttributeAdded { .. } => "AttributeAdded",
            Self::MediaAttached { .. } => "MediaAttached",
            Self::NoteAttached { .. } => "NoteAttached",
            Self::Tagged { .. } => "Tagged",
            Self::Untagged { .. } => "Untagged",
            Self::RestrictionsChanged { .. } => "RestrictionsChanged",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
            Self::HumanIdChanged { .. } => "HumanIdChanged",
        }
    }

    fn version(&self) -> &'static str {
        // Per-variant; bumped only on a payload change (ADR 0004 §4).
        // `MediaAttached` is "2.0" after `MediaRef.citations` widened to `EvidenceRef` (ADR 0023), no upcaster.
        match self {
            Self::MediaAttached { .. } => "2.0",
            _ => "1.0",
        }
    }
}
