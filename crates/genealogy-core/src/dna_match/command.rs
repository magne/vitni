//! `DnaMatch` commands — imperative operator intent (data-model §10, §12).

use crate::dna::{Centimorgans, DnaProvider, DnaSegment, PercentShared, SharedAncestor};
use crate::ids::{AssertionId, DnaMatchId, DnaTestId, HumanId, NoteId, TagId};
use crate::provenance::AssertionMeta;

/// Operator intent against a `DnaMatch` aggregate (data-model §10, §12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnaMatchCommand {
    /// Observe a match between two DNA tests (the create command).
    ObserveMatch {
        /// The application-generated id for the new match.
        dna_match_id: DnaMatchId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// One side's test.
        test_a: DnaTestId,
        /// The other side's test.
        test_b: DnaTestId,
        /// The provider the match was observed at (providers use different thresholds — §12).
        provider: DnaProvider,
        /// Total shared centimorgans.
        shared_cm: Centimorgans,
        /// Shared percentage, if reported.
        percent_shared: Option<PercentShared>,
        /// The number of shared segments.
        segment_count: u32,
        /// The largest shared segment's length.
        largest_segment_cm: Centimorgans,
        /// The provider's predicted relationship, if any.
        predicted_relationship: Option<String>,
    },
    /// Add a shared segment to the match.
    AddSegment {
        /// The target match.
        dna_match_id: DnaMatchId,
        /// The segment.
        segment: DnaSegment,
    },
    /// Assert an inferred common ancestor for the match.
    AssertSharedAncestor {
        /// The target match.
        dna_match_id: DnaMatchId,
        /// The shared ancestor.
        ancestor: SharedAncestor,
    },
    /// Confirm the match (a human-reviewed acceptance).
    ConfirmMatch {
        /// The target match.
        dna_match_id: DnaMatchId,
    },
    /// Reject the match.
    RejectMatch {
        /// The target match.
        dna_match_id: DnaMatchId,
    },
    /// Attach a note to the match.
    AttachNote {
        /// The target match.
        dna_match_id: DnaMatchId,
        /// The note to attach.
        note_id: NoteId,
    },
    /// Apply a tag to the match.
    Tag {
        /// The target match.
        dna_match_id: DnaMatchId,
        /// The tag to apply.
        tag_id: TagId,
    },
    /// Remove a tag from the match.
    Untag {
        /// The target match.
        dna_match_id: DnaMatchId,
        /// The tag to remove.
        tag_id: TagId,
    },
    /// Retract a prior assertion (non-destructive).
    RetractAssertion {
        /// The target match.
        dna_match_id: DnaMatchId,
        /// The assertion to retract.
        target: AssertionId,
    },
    /// Supersede a prior assertion with a replacement command.
    SupersedeAssertion {
        /// The target match.
        dna_match_id: DnaMatchId,
        /// The assertion to supersede.
        target: AssertionId,
        /// The command producing the replacement assertion.
        replacement: Box<DnaMatchCommand>,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the `DnaMatch` aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaMatchCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: DnaMatchCommand,
}
