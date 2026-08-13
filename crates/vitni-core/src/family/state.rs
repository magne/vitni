//! [`FamilyState`] — the folded aggregate state used by the decision core.
//!
//! This is the `cqrs-es` aggregate type: it must be `Default` (an unseen family) and serializable
//! (for snapshotting). It is rebuilt by replaying events through `evolve`. The asserted membership
//! (partners, children) is kept attributed to the [`AssertionId`] that introduced it, so a
//! retraction or supersession can remove exactly the right entry.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::{Asserted, Attributed};
use crate::enums::{ChildParentRelationship, Restriction};
use crate::ids::{AssertionId, CitationId, EventId, FamilyId, HumanId, NoteId, PersonId, TagId};
use crate::text::{ExternalId, MediaRef};

/// A child of the family with its parent relationships (data-model §6, §7).
///
/// The **read-model reconstruction** a [`FamilyView`](crate::family::FamilyView) returns: it is
/// rebuilt by folding the per-`(child, parent)` [`ChildRelationship`] rows against the child's
/// membership, not stored on a single assertion (ADR 0021). The relationship is recorded **per
/// family partner** (GEDCOM `_FREL`/`_MREL`): a child can be a birth child of one partner and a
/// step/adopted child of another. Remarriage and adoption over time are modelled as a *separate
/// family* (a person is a child in multiple families), never as more than the family's partners.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildEntry {
    /// The child person.
    pub child_id: PersonId,
    /// How the child relates to each family partner, by `PersonId`. A partner absent from this list
    /// has an unspecified relationship to the child.
    pub relationships: Vec<(PersonId, ChildParentRelationship)>,
}

/// One child-to-partner relationship, asserted on its own (GEDCOM `_FREL`/`_MREL` — ADR 0021).
///
/// Each `(child_id, parent_id)` pair is a separate assertion so an adoption link can be retracted or
/// re-cited without touching the child's membership or the other links. Folded as
/// `Attributed<Asserted<ChildRelationship>>` so a read model surfaces the link's surety + source
/// count per row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ChildRelationship {
    /// The child person.
    pub child_id: PersonId,
    /// The family partner the relationship is to.
    pub parent_id: PersonId,
    /// How the child relates to that partner.
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
    /// The family's privacy restrictions (GEDCOM `RESN`, last writer wins — data-model §6).
    pub restrictions: BTreeSet<Restriction>,
    /// All currently-live partner participations (neutral roles, GEDCOM `FAM.HUSB`/`WIFE`), each the
    /// asserted partner `PersonId` with its denormalized provenance (surety + backing citations).
    pub partners: Vec<Attributed<Asserted<PersonId>>>,
    /// All currently-live children (membership only, GEDCOM `FAM.CHIL`), each the asserted child
    /// `PersonId` with its provenance; the per-partner relationships are separate
    /// [`ChildRelationship`] rows (ADR 0021).
    pub children: Vec<Attributed<Asserted<PersonId>>>,
    /// All currently-live child–parent relationship rows, each with its provenance (ADR 0021).
    pub child_relationships: Vec<Attributed<Asserted<ChildRelationship>>>,
    /// All currently-live citations backing the family's claims (e.g. `FAM.SOUR`).
    pub citations: Vec<Attributed<CitationId>>,
    /// All currently-live linked family events (e.g. a marriage `Event` — `FAM.MARR`), each the
    /// asserted `EventId` with its provenance.
    pub linked_events: Vec<Attributed<Asserted<EventId>>>,
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
        self.partners.iter().any(|p| p.value.value == person_id)
    }

    /// Whether `child_id` is a currently-live child (member) of the family.
    #[must_use]
    pub(crate) fn has_child(&self, child_id: PersonId) -> bool {
        self.children.iter().any(|c| c.value.value == child_id)
    }

    /// Whether a live relationship for this `(child, parent)` pair already exists.
    #[must_use]
    pub(crate) fn has_child_relationship(&self, child_id: PersonId, parent_id: PersonId) -> bool {
        self.child_relationships
            .iter()
            .any(|r| r.value.value.child_id == child_id && r.value.value.parent_id == parent_id)
    }

    /// Whether an external id with this `(authority, value)` is currently live.
    #[must_use]
    pub(crate) fn has_external_id(&self, authority: &str, value: &str) -> bool {
        self.external_ids
            .iter()
            .any(|e| e.value.authority == authority && e.value.value == value)
    }

    /// Drops every relationship row for `child_id` (and its assertion ids from the live set) — the
    /// cascade a child's removal or membership-retraction triggers (ADR 0021).
    pub(crate) fn remove_child_rows(&mut self, child_id: PersonId) {
        for row in &self.child_relationships {
            if row.value.value.child_id == child_id {
                self.live_assertions.remove(&row.assertion_id);
            }
        }
        self.child_relationships.retain(|r| r.value.value.child_id != child_id);
    }

    /// Removes every value introduced by `target` and drops it from the live set.
    ///
    /// This is the non-destructive-correction fold: the *event log* keeps the original assertion
    /// forever, but the derived state no longer reflects the retracted claim. Retracting a child's
    /// **membership** cascades that child's relationship rows (ADR 0021); the removed child is
    /// captured *before* the retain so the cascade can still find it. Retracting a single
    /// relationship row touches only that row (the plain `child_relationships` retain).
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        let removed_child = self
            .children
            .iter()
            .find(|c| c.assertion_id == target)
            .map(|c| c.value.value);
        self.partners.retain(|p| p.assertion_id != target);
        self.children.retain(|c| c.assertion_id != target);
        self.child_relationships.retain(|r| r.assertion_id != target);
        self.citations.retain(|c| c.assertion_id != target);
        self.linked_events.retain(|e| e.assertion_id != target);
        self.media.retain(|m| m.assertion_id != target);
        self.notes.retain(|n| n.assertion_id != target);
        self.tags.retain(|t| t.assertion_id != target);
        self.external_ids.retain(|e| e.assertion_id != target);
        self.live_assertions.remove(&target);
        if let Some(child_id) = removed_child {
            self.remove_child_rows(child_id);
        }
    }
}
