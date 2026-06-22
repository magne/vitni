//! [`PersonState`] — the folded aggregate state used by the decision core.
//!
//! This is the `cqrs-es` aggregate type: it must be `Default` (an unseen person) and serializable
//! (for snapshotting). It is rebuilt by replaying events through `evolve`. Conclusion-layer fields
//! that are *asserted* (names, sex, facts) are kept attributed to the [`AssertionId`] that
//! introduced them, so a retraction or supersession can remove exactly the right entry.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::enums::{AssociationRole, EvidenceLevel, ParticipantRole, Sex};
use crate::fact::Fact;
use crate::ids::{AssertionId, CitationId, EventId, HumanId, NoteId, PersonId, TagId};
use crate::name::PersonName;
use crate::text::{ExternalId, MediaRef};

/// A person-to-person association (GEDCOM 7 `ASSO` — data-model §10): the associated person and the
/// role they play (a godparent, witness, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Association {
    /// The associated person.
    pub other: PersonId,
    /// The kind of association.
    pub role: AssociationRole,
}

/// A person's participation in a shared event (data-model §6, §10): the event and the role the
/// person played in it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participation {
    /// The event participated in.
    pub event_id: EventId,
    /// The participant's role.
    pub role: ParticipantRole,
}

/// The folded state of a Person aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct PersonState {
    /// Whether `PersonCreated` has been seen.
    pub exists: bool,
    /// The person's id (set on creation).
    pub person_id: Option<PersonId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// Whether this is a persona or a conclusion.
    pub evidence_level: Option<EvidenceLevel>,
    /// The most recently asserted sex (last writer wins).
    pub sex: Option<Attributed<Sex>>,
    /// All currently-live asserted names.
    pub names: Vec<Attributed<PersonName>>,
    /// All currently-live asserted facts.
    pub facts: Vec<Attributed<Fact>>,
    /// All currently-live asserted person-to-person associations (data-model §10).
    pub associations: Vec<Attributed<Association>>,
    /// All currently-live asserted event participations (data-model §6, §10).
    pub participations: Vec<Attributed<Participation>>,
    /// All currently-live citations backing the person's claims (e.g. `INDI.SOUR`).
    pub citations: Vec<Attributed<CitationId>>,
    /// All currently-live attached media (e.g. `INDI.OBJE`).
    pub media: Vec<Attributed<MediaRef>>,
    /// All currently-live attached notes (e.g. `INDI.NOTE`).
    pub notes: Vec<Attributed<NoteId>>,
    /// All currently-applied tags.
    pub tags: Vec<Attributed<TagId>>,
    /// Whether the person is marked private.
    pub private: bool,
    /// Persons merged into this surviving person (data-model §9).
    pub merged: Vec<PersonId>,
    /// All currently-live external identifiers (data-model §11) — the re-import resolution key.
    pub external_ids: Vec<Attributed<ExternalId>>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1 `RetractsMissingAssertion`).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl PersonState {
    /// Whether an external id with this `(authority, value)` is currently live.
    #[must_use]
    pub(crate) fn has_external_id(&self, authority: &str, value: &str) -> bool {
        self.external_ids
            .iter()
            .any(|e| e.value.authority == authority && e.value.value == value)
    }

    /// Removes every value introduced by `target` and drops it from the live set.
    ///
    /// This is the non-destructive-correction fold: the *event log* keeps the original
    /// assertion forever, but the derived state no longer reflects the retracted claim.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        self.names.retain(|n| n.assertion_id != target);
        self.facts.retain(|f| f.assertion_id != target);
        self.associations.retain(|a| a.assertion_id != target);
        self.participations.retain(|p| p.assertion_id != target);
        self.citations.retain(|c| c.assertion_id != target);
        self.media.retain(|m| m.assertion_id != target);
        self.notes.retain(|n| n.assertion_id != target);
        self.tags.retain(|t| t.assertion_id != target);
        self.external_ids.retain(|e| e.assertion_id != target);
        if self.sex.as_ref().is_some_and(|s| s.assertion_id == target) {
            self.sex = None;
        }
        self.live_assertions.remove(&target);
    }
}
