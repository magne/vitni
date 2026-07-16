//! Family events — the past-tense assertions the aggregate produces (data-model §10).
//!
//! Each event is wrapped in an envelope ([`FamilyEvent`]) that carries the [`AssertionId`] and the
//! [`EventContext`] in the payload (ADR 0004 §1–§2) — keeping provenance out of `cqrs-es` metadata
//! and out of every individual variant. The body is internally tagged (`type` discriminator) and
//! flattened into the envelope, so a stored event is a single flat JSON object with its `type`
//! (ADR 0004 §4).

use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::{Envelope, EventBody};
use crate::enums::{ChildParentRelationship, Restriction};
use crate::ids::{AssertionId, CitationId, EventId, FamilyId, HumanId, NoteId, PersonId, TagId};
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
    /// A child was added to the family (membership only — ADR 0021). Each parent relationship is a
    /// separate [`FamilyEventBody::ChildRelationshipAsserted`].
    ChildAdded {
        /// The family.
        family_id: FamilyId,
        /// The child added.
        child_id: PersonId,
    },
    /// A child's relationship to one family partner was asserted (GEDCOM `_FREL`/`_MREL` — ADR 0021).
    /// Its own envelope/`AssertionId`, so it corrects independently of membership and the other links.
    ChildRelationshipAsserted {
        /// The family.
        family_id: FamilyId,
        /// The child.
        child_id: PersonId,
        /// The family partner the relationship is to.
        parent_id: PersonId,
        /// How the child relates to that partner.
        relationship: ChildParentRelationship,
    },
    /// A child was removed from the family.
    ChildRemoved {
        /// The family.
        family_id: FamilyId,
        /// The child removed.
        child_id: PersonId,
    },
    /// The family's privacy restrictions were set / changed (GEDCOM `RESN` — data-model §6).
    RestrictionsChanged {
        /// The family.
        family_id: FamilyId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// A citation backing the family's claims was added.
    CitationAdded {
        /// The family.
        family_id: FamilyId,
        /// The added citation.
        citation_id: CitationId,
    },
    /// A family event (an `Event` aggregate, e.g. a marriage) was linked to the family.
    FamilyEventLinked {
        /// The family.
        family_id: FamilyId,
        /// The linked event.
        event_id: EventId,
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
    /// The family's user-facing identifier was changed (data-model §7).
    HumanIdChanged {
        /// The family.
        family_id: FamilyId,
        /// The new user-facing identifier.
        human_id: HumanId,
        /// The identifier in effect before this change (for the audit trail).
        old_human_id: HumanId,
    },
}

impl EventBody for FamilyEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::FamilyCreated { .. } => "FamilyCreated",
            Self::PartnerAdded { .. } => "PartnerAdded",
            Self::PartnerRemoved { .. } => "PartnerRemoved",
            Self::ChildAdded { .. } => "ChildAdded",
            Self::ChildRelationshipAsserted { .. } => "ChildRelationshipAsserted",
            Self::ChildRemoved { .. } => "ChildRemoved",
            Self::RestrictionsChanged { .. } => "RestrictionsChanged",
            Self::CitationAdded { .. } => "CitationAdded",
            Self::FamilyEventLinked { .. } => "FamilyEventLinked",
            Self::MediaAttached { .. } => "MediaAttached",
            Self::NoteAttached { .. } => "NoteAttached",
            Self::Tagged { .. } => "Tagged",
            Self::Untagged { .. } => "Untagged",
            Self::ExternalIdAdded { .. } => "ExternalIdAdded",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
            Self::HumanIdChanged { .. } => "HumanIdChanged",
        }
    }

    fn version(&self) -> &'static str {
        // Per-variant; bumped only on a payload change (ADR 0004 §4).
        // `ChildAdded` is "2.0" after shedding `relationships` to the new per-link
        // `ChildRelationshipAsserted` (ADR 0021), no upcaster; every other variant stays "1.0".
        // `MediaAttached` is "2.0" after `MediaRef.citations` widened to `EvidenceRef` (ADR 0023), no upcaster.
        match self {
            Self::ChildAdded { .. } | Self::MediaAttached { .. } => "2.0",
            _ => "1.0",
        }
    }
}
