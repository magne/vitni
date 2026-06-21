//! Citation commands — imperative operator intent (data-model §10).

use crate::date::GenealogicalDate;
use crate::ids::{AssertionId, CitationId, HumanId, NoteId, SourceId, TagId};
use crate::provenance::{AssertionMeta, Confidence, EvidenceAnalysis};
use crate::text::{Attribute, MediaRef};

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
    /// Assert the date of the cited record.
    AssertDate {
        /// The target citation.
        citation_id: CitationId,
        /// The date.
        date: GenealogicalDate,
    },
    /// Set (or change) the operator's confidence in this citation.
    SetConfidence {
        /// The target citation.
        citation_id: CitationId,
        /// The confidence level.
        confidence: Confidence,
    },
    /// Set (or change) the citation's evidence analysis (the *Evidence Explained* axes).
    SetEvidenceAnalysis {
        /// The target citation.
        citation_id: CitationId,
        /// The evidence analysis.
        analysis: EvidenceAnalysis,
    },
    /// Add a typed attribute to the citation.
    AddAttribute {
        /// The target citation.
        citation_id: CitationId,
        /// The attribute.
        attribute: Attribute,
    },
    /// Attach a media reference to the citation.
    AttachMedia {
        /// The target citation.
        citation_id: CitationId,
        /// The media reference.
        media: MediaRef,
    },
    /// Attach a note to the citation.
    AttachNote {
        /// The target citation.
        citation_id: CitationId,
        /// The note to attach.
        note_id: NoteId,
    },
    /// Apply a tag to the citation.
    Tag {
        /// The target citation.
        citation_id: CitationId,
        /// The tag to apply.
        tag_id: TagId,
    },
    /// Remove a tag from the citation.
    Untag {
        /// The target citation.
        citation_id: CitationId,
        /// The tag to remove.
        tag_id: TagId,
    },
    /// Retract a prior assertion (non-destructive).
    RetractAssertion {
        /// The target citation.
        citation_id: CitationId,
        /// The assertion to retract.
        target: AssertionId,
    },
    /// Supersede a prior assertion with a replacement command.
    SupersedeAssertion {
        /// The target citation.
        citation_id: CitationId,
        /// The assertion to supersede.
        target: AssertionId,
        /// The command producing the replacement assertion.
        replacement: Box<CitationCommand>,
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
