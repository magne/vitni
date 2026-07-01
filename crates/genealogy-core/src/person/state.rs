//! [`PersonState`] — the folded aggregate state used by the decision core.
//!
//! This is the `cqrs-es` aggregate type: it must be `Default` (an unseen person) and serializable
//! (for snapshotting). It is rebuilt by replaying events through `evolve`. Conclusion-layer fields
//! that are *asserted* (names, sex, facts) are kept attributed to the [`AssertionId`] that
//! introduced them, so a retraction or supersession can remove exactly the right entry.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::enums::{AssociationRole, EvidenceLevel, ParticipantRole, Restriction, Sex};
use crate::fact::Fact;
use crate::ids::{AssertionId, CitationId, EventId, HumanId, NoteId, PersonId, TagId};
use crate::name::PersonName;
use crate::provenance::Confidence;
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

/// An asserted name together with the provenance the asserting operator stamped on it.
///
/// Mirrors [`AssertedFact`]: the [`Confidence`] and the backing citation ids are denormalized from
/// the assertion's `EventContext` at fold time (ADR 0004 §1), so a read model can surface a name's
/// surety + source count per row without re-reading the log.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertedName {
    /// The asserted name (data-model §7).
    pub name: PersonName,
    /// The operator's surety when asserting it (data-model §8).
    pub confidence: Confidence,
    /// The citations backing the name (`EventContext.citations`).
    pub citations: Vec<CitationId>,
}

/// An asserted person-to-person association with the provenance stamped on it (see [`AssertedName`]).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertedAssociation {
    /// The asserted association (data-model §10).
    pub association: Association,
    /// The operator's surety when asserting it (data-model §8).
    pub confidence: Confidence,
    /// The citations backing the association (`EventContext.citations`).
    pub citations: Vec<CitationId>,
}

/// An asserted fact together with the confidence the asserting operator stamped on it.
///
/// The fact's claim lives in [`Fact`]; the [`Confidence`] is denormalized from the assertion's
/// `EventContext` at fold time (ADR 0004 §1 — confidence stays in the envelope on the event; the
/// projection copies it so a read model can surface it per fact without re-reading the log).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct AssertedFact {
    /// The asserted fact (INDI attribute — data-model §7).
    pub fact: Fact,
    /// The operator's surety when asserting it (data-model §8).
    pub confidence: Confidence,
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
    /// All currently-live asserted names, each with its assertion-time provenance.
    pub names: Vec<Attributed<AssertedName>>,
    /// All currently-live asserted facts, each with its assertion-time confidence.
    pub facts: Vec<Attributed<AssertedFact>>,
    /// All currently-live asserted person-to-person associations, each with its provenance (§10).
    pub associations: Vec<Attributed<AssertedAssociation>>,
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
        if self.sex.as_ref().is_some_and(|s| s.assertion_id == target) {
            self.sex = None;
        }
        if self.restrictions_assertion == Some(target) {
            self.restrictions.clear();
            self.restrictions_assertion = None;
        }
        self.live_assertions.remove(&target);
    }
}
