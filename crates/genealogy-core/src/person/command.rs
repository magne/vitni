//! Person commands — imperative operator intent (data-model §10).
//!
//! Commands are *pure intent*: they carry no clock, generated id, or operator. The application
//! layer builds an [`AssertionMeta`] (with the pre-generated [`crate::ids::AssertionId`] and the
//! [`crate::provenance::EventContext`]) and pairs it with the command in a [`PersonCommandEnvelope`]
//! before the pure `decide` core runs (ADR 0004 §3).

use crate::enums::{AssociationRole, EvidenceLevel, ParticipantRole, Sex};
use crate::fact::Fact;
use crate::ids::{AssertionId, EventId, HumanId, NoteId, PersonId, TagId};
use crate::name::PersonName;
use crate::provenance::AssertionMeta;
use crate::text::{ExternalId, MediaRef};

/// Operator intent against a Person aggregate (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PersonCommand {
    /// Create a new person (persona or conclusion).
    CreatePerson {
        /// The application-generated id for the new person.
        person_id: PersonId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// Whether this is a persona or a conclusion.
        evidence_level: EvidenceLevel,
    },
    /// Assert a name.
    AssertName {
        /// The target person.
        person_id: PersonId,
        /// The name to assert.
        name: PersonName,
    },
    /// Assert the person's sex.
    AssertSex {
        /// The target person.
        person_id: PersonId,
        /// The sex to assert.
        sex: Sex,
    },
    /// Assert a single-person fact.
    AssertFact {
        /// The target person.
        person_id: PersonId,
        /// The fact to assert.
        fact: Fact,
    },
    /// Assert participation in a shared event.
    AssertParticipation {
        /// The target person.
        person_id: PersonId,
        /// The event participated in.
        event_id: EventId,
        /// The participant's role.
        role: ParticipantRole,
    },
    /// Assert an association to another person.
    AssertAssociation {
        /// The asserting person.
        person_id: PersonId,
        /// The associated person.
        other: PersonId,
        /// The kind of association.
        role: AssociationRole,
    },
    /// Attach media.
    AttachMedia {
        /// The target person.
        person_id: PersonId,
        /// The media use.
        media: MediaRef,
    },
    /// Attach a note.
    AttachNote {
        /// The target person.
        person_id: PersonId,
        /// The note to attach.
        note_id: NoteId,
    },
    /// Record a stable external identifier (idempotent — re-adding the same `(authority, value)`
    /// is a no-op). The resolution key that makes re-import idempotent (data-model §11).
    AddExternalId {
        /// The target person.
        person_id: PersonId,
        /// The external identifier to record.
        external_id: ExternalId,
    },
    /// Apply a tag.
    Tag {
        /// The target person.
        person_id: PersonId,
        /// The tag to apply.
        tag_id: TagId,
    },
    /// Remove a tag.
    Untag {
        /// The target person.
        person_id: PersonId,
        /// The tag to remove.
        tag_id: TagId,
    },
    /// Set the privacy flag.
    SetPrivacy {
        /// The target person.
        person_id: PersonId,
        /// The new privacy state.
        private: bool,
    },
    /// Retract a prior assertion (non-destructive).
    RetractAssertion {
        /// The target person.
        person_id: PersonId,
        /// The assertion to retract.
        target: AssertionId,
    },
    /// Supersede a prior assertion with a replacement command.
    SupersedeAssertion {
        /// The target person.
        person_id: PersonId,
        /// The assertion to supersede.
        target: AssertionId,
        /// The command producing the replacement assertion.
        replacement: Box<PersonCommand>,
    },
    /// Conclude that two persons are the same individual (non-destructive merge).
    MergePersons {
        /// The surviving (conclusion) person.
        surviving: PersonId,
        /// The person merged into the survivor.
        merged: PersonId,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the Person aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PersonCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: PersonCommand,
}
