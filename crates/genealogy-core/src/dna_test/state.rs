//! [`DnaTestState`] — the folded aggregate state used by the decision core.
//!
//! Provider/kit/test-type/genome-build are last-writer-wins; haplogroups accumulate. Each is
//! attributed to the [`AssertionId`] that introduced it. Notes and tags register only in
//! `live_assertions` (the Person precedent — ADR 0009 §4).

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::assertions::Attributed;
use crate::dna::{DnaGenomeBuild, DnaProvider, DnaTestType};
use crate::ids::{AssertionId, DnaTestId, HumanId, PersonId};

/// The folded state of a `DnaTest` aggregate (data-model §6, §12).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct DnaTestState {
    /// Whether `DnaTestCreated` has been seen.
    pub exists: bool,
    /// The test's id (set on creation).
    pub dna_test_id: Option<DnaTestId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// The person this test belongs to (set on creation).
    pub person_id: Option<PersonId>,
    /// The testing provider (last writer wins).
    pub provider: Option<Attributed<DnaProvider>>,
    /// The provider's kit id (last writer wins).
    pub kit_id: Option<Attributed<String>>,
    /// The test type (last writer wins).
    pub test_type: Option<Attributed<DnaTestType>>,
    /// The genome build (last writer wins).
    pub genome_build: Option<Attributed<DnaGenomeBuild>>,
    /// All currently-live haplogroups, in assertion order.
    pub haplogroups: Vec<Attributed<String>>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl DnaTestState {
    /// Removes every value introduced by `target` and drops it from the live set.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        self.haplogroups.retain(|h| h.assertion_id != target);
        if self.provider.as_ref().is_some_and(|p| p.assertion_id == target) {
            self.provider = None;
        }
        if self.kit_id.as_ref().is_some_and(|k| k.assertion_id == target) {
            self.kit_id = None;
        }
        if self.test_type.as_ref().is_some_and(|t| t.assertion_id == target) {
            self.test_type = None;
        }
        if self.genome_build.as_ref().is_some_and(|g| g.assertion_id == target) {
            self.genome_build = None;
        }
        self.live_assertions.remove(&target);
    }
}
