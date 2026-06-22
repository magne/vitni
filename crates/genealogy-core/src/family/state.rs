//! [`FamilyState`] — the folded aggregate state used by the decision core.
//!
//! This is the `cqrs-es` aggregate type: it must be `Default` (an unseen family) and serializable
//! (for snapshotting). It is rebuilt by replaying events through `evolve`. The asserted membership
//! (partners, children) is kept attributed to the [`AssertionId`] that introduced it, so a
//! retraction or supersession can remove exactly the right entry.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::enums::ChildParentRelationship;
use crate::ids::{AssertionId, CitationId, FamilyId, HumanId, NoteId, PersonId, TagId};
use crate::text::{ExternalId, MediaRef};

/// A child of the family with its parent relationship (data-model §6, §7).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildEntry {
    /// The child person.
    pub child_id: PersonId,
    /// How the child relates to the family's parents.
    pub relationship: ChildParentRelationship,
}

/// The folded state of a Family aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct FamilyState {
    /// Whether `FamilyCreated` has been seen.
    pub exists: bool,
    /// The family's id (set on creation).
    pub family_id: Option<FamilyId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// Whether the family is marked private.
    pub private: bool,
    /// All currently-live partner participations (neutral roles).
    pub partners: Vec<Attributed<PersonId>>,
    /// All currently-live children.
    pub children: Vec<Attributed<ChildEntry>>,
    /// All currently-live citations backing the family's claims (e.g. `FAM.SOUR`).
    pub citations: Vec<Attributed<CitationId>>,
    /// All currently-live attached media (e.g. `FAM.OBJE`).
    pub media: Vec<Attributed<MediaRef>>,
    /// All currently-live attached notes (e.g. `FAM.NOTE`).
    pub notes: Vec<Attributed<NoteId>>,
    /// All currently-applied tags.
    pub tags: Vec<Attributed<TagId>>,
    /// All currently-live external identifiers (data-model §11) — the re-import resolution key.
    pub external_ids: Vec<Attributed<ExternalId>>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl FamilyState {
    /// Whether `person_id` is a currently-live partner.
    #[must_use]
    pub(crate) fn has_partner(&self, person_id: PersonId) -> bool {
        self.partners.iter().any(|p| p.value == person_id)
    }

    /// Whether `child_id` is a currently-live child.
    #[must_use]
    pub(crate) fn has_child(&self, child_id: PersonId) -> bool {
        self.children.iter().any(|c| c.value.child_id == child_id)
    }

    /// Whether an external id with this `(authority, value)` is currently live.
    #[must_use]
    pub(crate) fn has_external_id(&self, authority: &str, value: &str) -> bool {
        self.external_ids
            .iter()
            .any(|e| e.value.authority == authority && e.value.value == value)
    }

    /// Removes every value introduced by `target` and drops it from the live set.
    ///
    /// This is the non-destructive-correction fold: the *event log* keeps the original assertion
    /// forever, but the derived state no longer reflects the retracted claim.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        self.partners.retain(|p| p.assertion_id != target);
        self.children.retain(|c| c.assertion_id != target);
        self.citations.retain(|c| c.assertion_id != target);
        self.media.retain(|m| m.assertion_id != target);
        self.notes.retain(|n| n.assertion_id != target);
        self.tags.retain(|t| t.assertion_id != target);
        self.external_ids.retain(|e| e.assertion_id != target);
        self.live_assertions.remove(&target);
    }
}
