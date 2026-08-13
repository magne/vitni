//! Person events — the past-tense assertions the aggregate produces (data-model §10).
//!
//! Each event is wrapped in an envelope ([`PersonEvent`]) that carries the [`AssertionId`] and the
//! [`EventContext`] in the payload (ADR 0004 §1–§2) — keeping provenance out of `cqrs-es` metadata
//! and out of every individual variant. The body is internally tagged (`type` discriminator) and
//! flattened into the envelope, so a stored event is a single flat JSON object with its `type`
//! (ADR 0004 §4).

use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::age::Age;
use crate::assertions::{Envelope, EventBody};
use crate::enums::{AssociationRole, EvidenceLevel, ParticipantRole, Restriction, Sex};
use crate::fact::Fact;
use crate::ids::{AssertionId, CitationId, EventId, HumanId, NoteId, PersonId, TagId};
use crate::name::PersonName;
use crate::text::{Attribute, ExternalId, MediaRef};

/// A single Person assertion plus its provenance envelope (ADR 0004 §1).
pub type PersonEvent = Envelope<PersonEventBody>;

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
        /// The participant's age at the event, if recorded (ADR 0019).
        age: Option<Age>,
        /// Participant-scoped typed attributes (ADR 0019).
        attributes: Vec<Attribute>,
        /// Notes about this participation (ADR 0019).
        notes: Vec<NoteId>,
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
    /// A citation backing the person's claims was added.
    CitationAdded {
        /// The person.
        person_id: PersonId,
        /// The added citation.
        citation_id: CitationId,
    },
    /// A stable external identifier was recorded (data-model §11).
    ExternalIdAdded {
        /// The person.
        person_id: PersonId,
        /// The recorded external identifier.
        external_id: ExternalId,
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
    /// The person's privacy restrictions were set / changed (GEDCOM `RESN` — data-model §6).
    RestrictionsChanged {
        /// The person.
        person_id: PersonId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// The person's user-facing identifier was changed (data-model §7).
    HumanIdChanged {
        /// The person.
        person_id: PersonId,
        /// The new user-facing identifier.
        human_id: HumanId,
        /// The identifier in effect before this change (for the audit trail).
        old_human_id: HumanId,
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

impl EventBody for PersonEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::PersonCreated { .. } => "PersonCreated",
            Self::NameAsserted { .. } => "NameAsserted",
            Self::SexAsserted { .. } => "SexAsserted",
            Self::FactAsserted { .. } => "FactAsserted",
            Self::ParticipationAsserted { .. } => "ParticipationAsserted",
            Self::AssociationAsserted { .. } => "AssociationAsserted",
            Self::MediaAttached { .. } => "MediaAttached",
            Self::NoteAttached { .. } => "NoteAttached",
            Self::CitationAdded { .. } => "CitationAdded",
            Self::ExternalIdAdded { .. } => "ExternalIdAdded",
            Self::Tagged { .. } => "Tagged",
            Self::Untagged { .. } => "Untagged",
            Self::RestrictionsChanged { .. } => "RestrictionsChanged",
            Self::HumanIdChanged { .. } => "HumanIdChanged",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
            Self::PersonsMerged { .. } => "PersonsMerged",
        }
    }

    fn version(&self) -> &'static str {
        // Per-variant; bumped only on an incompatible payload change (ADR 0004 §4).
        // `FactAsserted` is "2.0" after dropping `Fact.citations` (ADR 0020), no upcaster.
        // `ParticipationAsserted` is "2.0" after gaining age/attributes/notes (ADR 0019), no upcaster.
        // `MediaAttached` is "2.0" after `MediaRef.citations` widened to `EvidenceRef` (ADR 0023), no upcaster.
        match self {
            Self::FactAsserted { .. } | Self::ParticipationAsserted { .. } | Self::MediaAttached { .. } => "2.0",
            _ => "1.0",
        }
    }
}
