//! Person events — the past-tense assertions the aggregate produces (data-model §10).
//!
//! Each event is wrapped in an envelope ([`PersonEvent`]) that carries the [`AssertionId`] and the
//! [`EventContext`] in the payload (ADR 0004 §1–§2) — keeping provenance out of `cqrs-es` metadata
//! and out of every individual variant. The body is internally tagged (`type` discriminator) and
//! flattened into the envelope, so a stored event is a single flat JSON object with its `type`
//! (ADR 0004 §4).

use cqrs_es::DomainEvent;
use serde::{Deserialize, Serialize};

use crate::enums::{AssociationRole, EvidenceLevel, ParticipantRole, Sex};
use crate::fact::Fact;
use crate::ids::{AssertionId, EventId, HumanId, NoteId, PersonId, TagId};
use crate::name::PersonName;
use crate::provenance::{AssertionMeta, EventContext};
use crate::text::MediaRef;

/// A single Person assertion plus its provenance envelope (ADR 0004 §1).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonEvent {
    /// Identity of this assertion, so a correction can target it (ADR 0004 §2).
    pub assertion_id: AssertionId,
    /// Who / when / why / how sure / on what evidence (data-model §8).
    pub context: EventContext,
    /// The claim itself.
    #[serde(flatten)]
    pub body: PersonEventBody,
}

impl PersonEvent {
    /// Stamps `body` with the supplied assertion id and context (ADR 0004 §3).
    #[must_use]
    pub fn new(meta: &AssertionMeta, body: PersonEventBody) -> Self {
        Self {
            assertion_id: meta.assertion_id,
            context: meta.context.clone(),
            body,
        }
    }
}

/// The Person claim variants (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum PersonEventBody {
    /// A person aggregate was created (as a persona or a conclusion).
    PersonCreated {
        /// The created person.
        person_id: PersonId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// Whether this is a persona or a conclusion.
        evidence_level: EvidenceLevel,
    },
    /// A name was asserted for the person.
    NameAsserted {
        /// The person the name belongs to.
        person_id: PersonId,
        /// The asserted name.
        name: PersonName,
    },
    /// The person's sex was asserted.
    SexAsserted {
        /// The person.
        person_id: PersonId,
        /// The asserted sex.
        sex: Sex,
    },
    /// A single-person fact was asserted (birth, death, occupation, …).
    FactAsserted {
        /// The person.
        person_id: PersonId,
        /// The asserted fact.
        fact: Fact,
    },
    /// The person was linked as a participant in a shared event.
    ParticipationAsserted {
        /// The participating person.
        person_id: PersonId,
        /// The event participated in.
        event_id: EventId,
        /// The participant's role.
        role: ParticipantRole,
    },
    /// An association to another person was asserted.
    AssociationAsserted {
        /// The asserting person.
        person_id: PersonId,
        /// The associated person.
        other: PersonId,
        /// The kind of association.
        role: AssociationRole,
    },
    /// Media was attached to the person.
    MediaAttached {
        /// The person.
        person_id: PersonId,
        /// The media use.
        media: MediaRef,
    },
    /// A note was attached to the person.
    NoteAttached {
        /// The person.
        person_id: PersonId,
        /// The attached note.
        note_id: NoteId,
    },
    /// A tag was applied to the person.
    Tagged {
        /// The person.
        person_id: PersonId,
        /// The applied tag.
        tag_id: TagId,
    },
    /// A tag was removed from the person.
    Untagged {
        /// The person.
        person_id: PersonId,
        /// The removed tag.
        tag_id: TagId,
    },
    /// The person's privacy flag changed.
    PrivacyChanged {
        /// The person.
        person_id: PersonId,
        /// The new privacy state.
        private: bool,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The person.
        person_id: PersonId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The person.
        person_id: PersonId,
        /// The assertion being superseded.
        target: AssertionId,
    },
    /// Two persons were concluded to be the same individual (non-destructive — data-model §9).
    PersonsMerged {
        /// The surviving (conclusion) person.
        surviving: PersonId,
        /// The person merged into the survivor.
        merged: PersonId,
    },
}

impl PersonEventBody {
    /// The variant name, used as the `cqrs-es` event type (ADR 0004 §4).
    #[must_use]
    pub fn type_name(&self) -> &'static str {
        match self {
            Self::PersonCreated { .. } => "PersonCreated",
            Self::NameAsserted { .. } => "NameAsserted",
            Self::SexAsserted { .. } => "SexAsserted",
            Self::FactAsserted { .. } => "FactAsserted",
            Self::ParticipationAsserted { .. } => "ParticipationAsserted",
            Self::AssociationAsserted { .. } => "AssociationAsserted",
            Self::MediaAttached { .. } => "MediaAttached",
            Self::NoteAttached { .. } => "NoteAttached",
            Self::Tagged { .. } => "Tagged",
            Self::Untagged { .. } => "Untagged",
            Self::PrivacyChanged { .. } => "PrivacyChanged",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
            Self::PersonsMerged { .. } => "PersonsMerged",
        }
    }
}

impl DomainEvent for PersonEvent {
    fn event_type(&self) -> String {
        self.body.type_name().to_owned()
    }

    fn event_version(&self) -> String {
        // Bumped only on an incompatible payload change (ADR 0004 §4).
        "1.0".to_owned()
    }
}
