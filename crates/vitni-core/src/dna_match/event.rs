//! `DnaMatch` events — the past-tense assertions the aggregate produces (data-model §10, §12).

use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::{Envelope, EventBody};
use crate::dna::{Centimorgans, DnaProvider, DnaSegment, PercentShared, SharedAncestor};
use crate::enums::Restriction;
use crate::ids::{AssertionId, DnaMatchId, DnaTestId, HumanId, NoteId, TagId};

/// A single `DnaMatch` assertion plus its provenance envelope (ADR 0004 §1).
pub type DnaMatchEvent = Envelope<DnaMatchEventBody>;

/// The `DnaMatch` claim variants (data-model §10, §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DnaMatchEventBody {
    /// A match between two tests was observed (the create event).
    DnaMatchObserved {
        /// The created match.
        dna_match_id: DnaMatchId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// One side's test.
        test_a: DnaTestId,
        /// The other side's test.
        test_b: DnaTestId,
        /// The provider the match was observed at.
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
    /// A shared segment was added.
    SegmentAdded {
        /// The match.
        dna_match_id: DnaMatchId,
        /// The segment.
        segment: DnaSegment,
    },
    /// An inferred common ancestor was asserted.
    SharedAncestorAsserted {
        /// The match.
        dna_match_id: DnaMatchId,
        /// The shared ancestor.
        ancestor: SharedAncestor,
    },
    /// The match was confirmed.
    MatchConfirmed {
        /// The match.
        dna_match_id: DnaMatchId,
    },
    /// The match was rejected.
    MatchRejected {
        /// The match.
        dna_match_id: DnaMatchId,
    },
    /// A note was attached to the match.
    NoteAttached {
        /// The match.
        dna_match_id: DnaMatchId,
        /// The attached note.
        note_id: NoteId,
    },
    /// A tag was applied to the match.
    Tagged {
        /// The match.
        dna_match_id: DnaMatchId,
        /// The applied tag.
        tag_id: TagId,
    },
    /// A tag was removed from the match.
    Untagged {
        /// The match.
        dna_match_id: DnaMatchId,
        /// The removed tag.
        tag_id: TagId,
    },
    /// The match's privacy restrictions were set / changed (GEDCOM `RESN` — data-model §6).
    RestrictionsChanged {
        /// The match.
        dna_match_id: DnaMatchId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The match.
        dna_match_id: DnaMatchId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The match.
        dna_match_id: DnaMatchId,
        /// The assertion being superseded.
        target: AssertionId,
    },
    /// The DNA match's user-facing identifier was changed (data-model §7).
    HumanIdChanged {
        /// The match.
        dna_match_id: DnaMatchId,
        /// The new user-facing identifier.
        human_id: HumanId,
        /// The identifier in effect before this change (for the audit trail).
        old_human_id: HumanId,
    },
}

impl EventBody for DnaMatchEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::DnaMatchObserved { .. } => "DnaMatchObserved",
            Self::SegmentAdded { .. } => "SegmentAdded",
            Self::SharedAncestorAsserted { .. } => "SharedAncestorAsserted",
            Self::MatchConfirmed { .. } => "MatchConfirmed",
            Self::MatchRejected { .. } => "MatchRejected",
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
        "1.0"
    }
}
