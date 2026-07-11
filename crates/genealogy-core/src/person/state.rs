//! [`PersonState`] — the folded aggregate state used by the decision core.
//!
//! This is the `cqrs-es` aggregate type: it must be `Default` (an unseen person) and serializable
//! (for snapshotting). It is rebuilt by replaying events through `evolve`. Conclusion-layer fields
//! that are *asserted* (names, sex, facts) are kept attributed to the [`AssertionId`] that
//! introduced them, so a retraction or supersession can remove exactly the right entry.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::age::Age;
use crate::assertions::{Asserted, Attributed};
use crate::enums::{AssociationRole, EvidenceLevel, ParticipantRole, Restriction, Sex};
use crate::fact::Fact;
use crate::ids::{AssertionId, CitationId, EventId, HumanId, NoteId, PersonId, TagId};
use crate::name::PersonName;
use crate::text::{Attribute, ExternalId, MediaRef};

/// A person-to-person association (GEDCOM 7 `ASSO` — data-model §10): the associated person and the
/// role they play (a godparent, witness, …).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Association {
    /// The associated person.
    pub other: PersonId,
    /// The kind of association.
    pub role: AssociationRole,
}

/// A person's participation in a shared event (data-model §6, §10): the event, the role the person
/// played, and the participant-scoped detail a source records — the age at the event, typed
/// attributes, and notes (ADR 0019). Backing citations live on the assertion envelope, the sole
/// evidence channel (ADR 0020), not on the attributes or notes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Participation {
    /// The event participated in.
    pub event_id: EventId,
    /// The participant's role.
    pub role: ParticipantRole,
    /// The participant's age at the event, if recorded.
    pub age: Option<Age>,
    /// Participant-scoped typed attributes (e.g. a witness's recorded occupation).
    pub attributes: Vec<Attribute>,
    /// Notes about this participation.
    pub notes: Vec<NoteId>,
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
    /// All currently-live asserted sex values, each with its assertion-time provenance. Reads are
    /// last-live-wins (`PersonView::sex` returns the last), so retracting the latest restores the
    /// prior assertion instead of clearing the field (data-model §8).
    pub sex: Vec<Attributed<Asserted<Sex>>>,
    /// All currently-live asserted names, each with its assertion-time provenance.
    pub names: Vec<Attributed<Asserted<PersonName>>>,
    /// All currently-live asserted facts, each with its assertion-time confidence.
    pub facts: Vec<Attributed<Asserted<Fact>>>,
    /// All currently-live asserted person-to-person associations, each with its provenance (§10).
    pub associations: Vec<Attributed<Asserted<Association>>>,
    /// All currently-live asserted event participations, each with its assertion-time provenance
    /// (surety + backing citations denormalized from the envelope — data-model §6, §10; ADR 0019).
    pub participations: Vec<Attributed<Asserted<Participation>>>,
    /// All currently-live citations backing the person's claims (e.g. `INDI.SOUR`).
    pub citations: Vec<Attributed<CitationId>>,
    /// All currently-live attached media (e.g. `INDI.OBJE`).
    pub media: Vec<Attributed<MediaRef>>,
    /// All currently-live attached notes (e.g. `INDI.NOTE`).
    pub notes: Vec<Attributed<NoteId>>,
    /// All currently-applied tags.
    pub tags: Vec<Attributed<TagId>>,
    /// The person's privacy restrictions (GEDCOM `RESN`, last writer wins — data-model §6).
    pub restrictions: BTreeSet<Restriction>,
    /// The assertion that set the current `restrictions`, so retracting it clears them (the set is
    /// replaced wholesale, not accumulated, so it cannot be attributed per-element).
    #[serde(default)]
    pub restrictions_assertion: Option<AssertionId>,
    /// Persons merged into this surviving person (data-model §9), each attributed to the
    /// `PersonsMerged` assertion that recorded it, so undoing that assertion removes the persona link.
    pub merged: Vec<Attributed<PersonId>>,
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
        self.merged.retain(|m| m.assertion_id != target);
        self.sex.retain(|s| s.assertion_id != target);
        if self.restrictions_assertion == Some(target) {
            self.restrictions.clear();
            self.restrictions_assertion = None;
        }
        self.live_assertions.remove(&target);
    }
}
