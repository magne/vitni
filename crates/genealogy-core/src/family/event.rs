//! Family events — the past-tense assertions the aggregate produces (data-model §10).
//!
//! Each event is wrapped in an envelope ([`FamilyEvent`]) that carries the [`AssertionId`] and the
//! [`EventContext`] in the payload (ADR 0004 §1–§2) — keeping provenance out of `cqrs-es` metadata
//! and out of every individual variant. The body is internally tagged (`type` discriminator) and
//! flattened into the envelope, so a stored event is a single flat JSON object with its `type`
//! (ADR 0004 §4).

use cqrs_es::DomainEvent;
use serde::{Deserialize, Serialize};

use crate::enums::ChildParentRelationship;
use crate::ids::{AssertionId, FamilyId, HumanId, PersonId, TagId};
use crate::provenance::{AssertionMeta, EventContext};

/// A single Family assertion plus its provenance envelope (ADR 0004 §1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyEvent {
    /// Identity of this assertion, so a correction can target it (ADR 0004 §2).
    pub assertion_id: AssertionId,
    /// Who / when / why / how sure / on what evidence (data-model §8).
    pub context: EventContext,
    /// The claim itself.
    #[serde(flatten)]
    pub body: FamilyEventBody,
}

impl FamilyEvent {
    /// Stamps `body` with the supplied assertion id and context (ADR 0004 §3).
    #[must_use]
    pub fn new(meta: &AssertionMeta, body: FamilyEventBody) -> Self {
        Self {
            assertion_id: meta.assertion_id,
            context: meta.context.clone(),
            body,
        }
    }
}

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

impl FamilyEventBody {
    /// The variant name, used as the `cqrs-es` event type (ADR 0004 §4).
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::FamilyCreated { .. } => "FamilyCreated",
            Self::PartnerAdded { .. } => "PartnerAdded",
            Self::PartnerRemoved { .. } => "PartnerRemoved",
            Self::ChildAdded { .. } => "ChildAdded",
            Self::ChildRemoved { .. } => "ChildRemoved",
            Self::PrivacyChanged { .. } => "PrivacyChanged",
            Self::Tagged { .. } => "Tagged",
            Self::Untagged { .. } => "Untagged",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
        }
    }
}

impl DomainEvent for FamilyEvent {
    fn event_type(&self) -> String {
        self.body.type_name().to_owned()
    }

    fn event_version(&self) -> String {
        // Bumped only on an incompatible payload change (ADR 0004 §4).
        "1.0".to_owned()
    }
}
