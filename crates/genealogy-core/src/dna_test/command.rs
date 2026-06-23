//! `DnaTest` commands — imperative operator intent (data-model §10, §12).

use std::collections::BTreeSet;

use crate::dna::{DnaGenomeBuild, DnaProvider, DnaTestType};
use crate::enums::Restriction;
use crate::ids::{AssertionId, DnaTestId, HumanId, NoteId, PersonId, TagId};
use crate::provenance::AssertionMeta;

/// Operator intent against a `DnaTest` aggregate (data-model §10, §12).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DnaTestCommand {
    /// Create a new DNA test, anchored to one person.
    CreateDnaTest {
        /// The application-generated id for the new test.
        dna_test_id: DnaTestId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// The person this test belongs to (the cross-aggregate reference).
        person_id: PersonId,
    },
    /// Set (or change) the testing provider.
    SetProvider {
        /// The target test.
        dna_test_id: DnaTestId,
        /// The provider.
        provider: DnaProvider,
    },
    /// Set (or change) the provider's kit id.
    SetKitId {
        /// The target test.
        dna_test_id: DnaTestId,
        /// The kit id.
        kit_id: String,
    },
    /// Set (or change) the test type.
    SetTestType {
        /// The target test.
        dna_test_id: DnaTestId,
        /// The test type.
        test_type: DnaTestType,
    },
    /// Set (or change) the genome build.
    SetGenomeBuild {
        /// The target test.
        dna_test_id: DnaTestId,
        /// The genome build.
        genome_build: DnaGenomeBuild,
    },
    /// Assert a haplogroup observed in the test.
    AssertHaplogroup {
        /// The target test.
        dna_test_id: DnaTestId,
        /// The haplogroup.
        haplogroup: String,
    },
    /// Attach a note to the test.
    AttachNote {
        /// The target test.
        dna_test_id: DnaTestId,
        /// The note to attach.
        note_id: NoteId,
    },
    /// Apply a tag to the test.
    Tag {
        /// The target test.
        dna_test_id: DnaTestId,
        /// The tag to apply.
        tag_id: TagId,
    },
    /// Remove a tag from the test.
    Untag {
        /// The target test.
        dna_test_id: DnaTestId,
        /// The tag to remove.
        tag_id: TagId,
    },
    /// Set (or change) the test's privacy restrictions (GEDCOM `RESN` — data-model §6).
    SetRestrictions {
        /// The target test.
        dna_test_id: DnaTestId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// Retract a prior assertion (non-destructive).
    RetractAssertion {
        /// The target test.
        dna_test_id: DnaTestId,
        /// The assertion to retract.
        target: AssertionId,
    },
    /// Supersede a prior assertion with a replacement command.
    SupersedeAssertion {
        /// The target test.
        dna_test_id: DnaTestId,
        /// The assertion to supersede.
        target: AssertionId,
        /// The command producing the replacement assertion.
        replacement: Box<DnaTestCommand>,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the `DnaTest` aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DnaTestCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: DnaTestCommand,
}
