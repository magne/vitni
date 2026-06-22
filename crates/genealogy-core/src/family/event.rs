//! Family events — the past-tense assertions the aggregate produces (data-model §10).
//!
//! Each event is wrapped in an envelope ([`FamilyEvent`]) that carries the [`AssertionId`] and the
//! [`EventContext`] in the payload (ADR 0004 §1–§2) — keeping provenance out of `cqrs-es` metadata
//! and out of every individual variant. The body is internally tagged (`type` discriminator) and
//! flattened into the envelope, so a stored event is a single flat JSON object with its `type`
//! (ADR 0004 §4).

use serde::{Deserialize, Serialize};

use crate::assertions::{Envelope, EventBody};
use crate::enums::ChildParentRelationship;
use crate::ids::{AssertionId, CitationId, FamilyId, HumanId, NoteId, PersonId, TagId};
use crate::text::{ExternalId, MediaRef};

/// A single Family assertion plus its provenance envelope (ADR 0004 §1).
pub type FamilyEvent = Envelope<FamilyEventBody>;

/// The Family claim variants (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum FamilyEventBody {
    /// A family aggregate was created.
    FamilyCreated {
        /// The created family.
        family_id: FamilyId,
        /// The user-facing identifier.
        human_id: HumanId,
    },
    /// A partner was added to the family.
    PartnerAdded {
        /// The family.
        family_id: FamilyId,
        /// The partner added.
        person_id: PersonId,
    },
    /// A partner was removed from the family.
    PartnerRemoved {
        /// The family.
        family_id: FamilyId,
        /// The partner removed.
        person_id: PersonId,
    },
    /// A child was added to the family with its parent relationship.
    ChildAdded {
        /// The family.
        family_id: FamilyId,
        /// The child added.
        child_id: PersonId,
        /// How the child relates to the family's parents.
        relationship: ChildParentRelationship,
    },
    /// A child was removed from the family.
    ChildRemoved {
        /// The family.
        family_id: FamilyId,
        /// The child removed.
        child_id: PersonId,
    },
    /// The family's privacy flag changed.
    PrivacyChanged {
        /// The family.
        family_id: FamilyId,
        /// The new privacy state.
        private: bool,
    },
    /// A citation backing the family's claims was added.
    CitationAdded {
        /// The family.
        family_id: FamilyId,
        /// The added citation.
        citation_id: CitationId,
    },
    /// Media was attached to the family.
    MediaAttached {
        /// The family.
        family_id: FamilyId,
        /// The media use.
        media: MediaRef,
    },
    /// A note was attached to the family.
    NoteAttached {
        /// The family.
        family_id: FamilyId,
        /// The attached note.
        note_id: NoteId,
    },
    /// A tag was applied to the family.
    Tagged {
        /// The family.
        family_id: FamilyId,
        /// The applied tag.
        tag_id: TagId,
    },
    /// A tag was removed from the family.
    Untagged {
        /// The family.
        family_id: FamilyId,
        /// The removed tag.
        tag_id: TagId,
    },
    /// A stable external identifier was recorded (data-model §11).
    ExternalIdAdded {
        /// The family.
        family_id: FamilyId,
        /// The recorded external identifier.
        external_id: ExternalId,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The family.
        family_id: FamilyId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The family.
        family_id: FamilyId,
        /// The assertion being superseded.
        target: AssertionId,
    },
}

impl EventBody for FamilyEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::FamilyCreated { .. } => "FamilyCreated",
            Self::PartnerAdded { .. } => "PartnerAdded",
            Self::PartnerRemoved { .. } => "PartnerRemoved",
            Self::ChildAdded { .. } => "ChildAdded",
            Self::ChildRemoved { .. } => "ChildRemoved",
            Self::PrivacyChanged { .. } => "PrivacyChanged",
            Self::CitationAdded { .. } => "CitationAdded",
            Self::MediaAttached { .. } => "MediaAttached",
            Self::NoteAttached { .. } => "NoteAttached",
            Self::Tagged { .. } => "Tagged",
            Self::Untagged { .. } => "Untagged",
            Self::ExternalIdAdded { .. } => "ExternalIdAdded",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
        }
    }

    fn version(&self) -> &'static str {
        // Per-variant; bumped only on an additive payload change (ADR 0004 §4).
        "1.0"
    }
}
