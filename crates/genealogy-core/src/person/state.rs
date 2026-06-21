//! [`PersonState`] — the folded aggregate state used by the decision core.
//!
//! This is the `cqrs-es` aggregate type: it must be `Default` (an unseen person) and serializable
//! (for snapshotting). It is rebuilt by replaying events through `evolve`. Conclusion-layer fields
//! that are *asserted* (names, sex, facts) are kept attributed to the [`AssertionId`] that
//! introduced them, so a retraction or supersession can remove exactly the right entry.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::enums::{EvidenceLevel, Sex};
use crate::fact::Fact;
use crate::ids::{AssertionId, HumanId, PersonId};
use crate::name::PersonName;

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
    /// Whether the person is marked private.
    pub private: bool,
    /// Persons merged into this surviving person (data-model §9).
    pub merged: Vec<PersonId>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1 `RetractsMissingAssertion`).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl PersonState {
    /// Removes every value introduced by `target` and drops it from the live set.
    ///
    /// This is the non-destructive-correction fold: the *event log* keeps the original
    /// assertion forever, but the derived state no longer reflects the retracted claim.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        self.names.retain(|n| n.assertion_id != target);
        self.facts.retain(|f| f.assertion_id != target);
        if self.sex.as_ref().is_some_and(|s| s.assertion_id == target) {
            self.sex = None;
        }
        self.live_assertions.remove(&target);
    }
}
