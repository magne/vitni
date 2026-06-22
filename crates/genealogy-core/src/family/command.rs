//! Family commands — imperative operator intent (data-model §10).
//!
//! Commands are *pure intent*: they carry no clock, generated id, or operator. The application
//! layer builds an [`AssertionMeta`] (with the pre-generated [`crate::ids::AssertionId`] and the
//! [`crate::provenance::EventContext`]) and pairs it with the command in a [`FamilyCommandEnvelope`]
//! before the pure `decide` core runs (ADR 0004 §3).

use crate::enums::ChildParentRelationship;
use crate::ids::{AssertionId, CitationId, FamilyId, HumanId, NoteId, PersonId, TagId};
use crate::provenance::AssertionMeta;
use crate::text::{ExternalId, MediaRef};

/// Operator intent against a Family aggregate (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FamilyCommand {
    /// Create a new family.
    CreateFamily {
        /// The application-generated id for the new family.
        family_id: FamilyId,
        /// The user-facing identifier.
        human_id: HumanId,
    },
    /// Add a partner (neutral role) to the family.
    AddPartner {
        /// The target family.
        family_id: FamilyId,
        /// The partner to add.
        person_id: PersonId,
    },
    /// Remove a partner from the family.
    RemovePartner {
        /// The target family.
        family_id: FamilyId,
        /// The partner to remove.
        person_id: PersonId,
    },
    /// Add a child to the family with its parent relationship.
    AddChild {
        /// The target family.
        family_id: FamilyId,
        /// The child to add.
        child_id: PersonId,
        /// How the child relates to the family's parents.
        relationship: ChildParentRelationship,
    },
    /// Remove a child from the family.
    RemoveChild {
        /// The target family.
        family_id: FamilyId,
        /// The child to remove.
        child_id: PersonId,
    },
    /// Set the privacy flag.
    SetPrivacy {
        /// The target family.
        family_id: FamilyId,
        /// The new privacy state.
        private: bool,
    },
    /// Add a citation backing the family's claims (e.g. a GEDCOM `FAM.SOUR`).
    AddCitation {
        /// The target family.
        family_id: FamilyId,
        /// The citation to add.
        citation_id: CitationId,
    },
    /// Attach a media reference to the family (e.g. `FAM.OBJE`).
    AttachMedia {
        /// The target family.
        family_id: FamilyId,
        /// The media reference.
        media: MediaRef,
    },
    /// Attach a note to the family (e.g. `FAM.NOTE`).
    AttachNote {
        /// The target family.
        family_id: FamilyId,
        /// The note to attach.
        note_id: NoteId,
    },
    /// Apply a tag.
    Tag {
        /// The target family.
        family_id: FamilyId,
        /// The tag to apply.
        tag_id: TagId,
    },
    /// Remove a tag.
    Untag {
        /// The target family.
        family_id: FamilyId,
        /// The tag to remove.
        tag_id: TagId,
    },
    /// Retract a prior assertion (non-destructive).
    RetractAssertion {
        /// The target family.
        family_id: FamilyId,
        /// The assertion to retract.
        target: AssertionId,
    },
    /// Supersede a prior assertion with a replacement command.
    SupersedeAssertion {
        /// The target family.
        family_id: FamilyId,
        /// The assertion to supersede.
        target: AssertionId,
        /// The command producing the replacement assertion.
        replacement: Box<FamilyCommand>,
    },
    /// Record a stable external identifier (idempotent — re-adding the same `(authority, value)`
    /// is a no-op). The resolution key that makes re-import idempotent (data-model §11).
    AddExternalId {
        /// The target family.
        family_id: FamilyId,
        /// The external identifier to record.
        external_id: ExternalId,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the Family aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FamilyCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: FamilyCommand,
}
