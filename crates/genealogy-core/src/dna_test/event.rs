//! `DnaTest` events — the past-tense assertions the aggregate produces (data-model §10, §12).

use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::assertions::{Envelope, EventBody};
use crate::dna::{DnaGenomeBuild, DnaProvider, DnaTestType};
use crate::enums::Restriction;
use crate::ids::{AssertionId, DnaTestId, HumanId, NoteId, PersonId, TagId};

/// A single `DnaTest` assertion plus its provenance envelope (ADR 0004 §1).
pub type DnaTestEvent = Envelope<DnaTestEventBody>;

/// The `DnaTest` claim variants (data-model §10, §12).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "type")]
pub enum DnaTestEventBody {
    /// A DNA test aggregate was created, anchored to a person.
    DnaTestCreated {
        /// The created test.
        dna_test_id: DnaTestId,
        /// The user-facing identifier.
        human_id: HumanId,
        /// The person this test belongs to.
        person_id: PersonId,
    },
    /// The testing provider was set / changed.
    ProviderSet {
        /// The test.
        dna_test_id: DnaTestId,
        /// The provider.
        provider: DnaProvider,
    },
    /// The provider's kit id was set / changed.
    KitIdSet {
        /// The test.
        dna_test_id: DnaTestId,
        /// The kit id.
        kit_id: String,
    },
    /// The test type was set / changed.
    TestTypeSet {
        /// The test.
        dna_test_id: DnaTestId,
        /// The test type.
        test_type: DnaTestType,
    },
    /// The genome build was set / changed.
    GenomeBuildSet {
        /// The test.
        dna_test_id: DnaTestId,
        /// The genome build.
        genome_build: DnaGenomeBuild,
    },
    /// A haplogroup was asserted.
    HaplogroupAsserted {
        /// The test.
        dna_test_id: DnaTestId,
        /// The haplogroup.
        haplogroup: String,
    },
    /// A note was attached to the test.
    NoteAttached {
        /// The test.
        dna_test_id: DnaTestId,
        /// The attached note.
        note_id: NoteId,
    },
    /// A tag was applied to the test.
    Tagged {
        /// The test.
        dna_test_id: DnaTestId,
        /// The applied tag.
        tag_id: TagId,
    },
    /// A tag was removed from the test.
    Untagged {
        /// The test.
        dna_test_id: DnaTestId,
        /// The removed tag.
        tag_id: TagId,
    },
    /// The test's privacy restrictions were set / changed (GEDCOM `RESN` — data-model §6).
    RestrictionsChanged {
        /// The test.
        dna_test_id: DnaTestId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The test.
        dna_test_id: DnaTestId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The test.
        dna_test_id: DnaTestId,
        /// The assertion being superseded.
        target: AssertionId,
    },
}

impl EventBody for DnaTestEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::DnaTestCreated { .. } => "DnaTestCreated",
            Self::ProviderSet { .. } => "ProviderSet",
            Self::KitIdSet { .. } => "KitIdSet",
            Self::TestTypeSet { .. } => "TestTypeSet",
            Self::GenomeBuildSet { .. } => "GenomeBuildSet",
            Self::HaplogroupAsserted { .. } => "HaplogroupAsserted",
            Self::NoteAttached { .. } => "NoteAttached",
            Self::Tagged { .. } => "Tagged",
            Self::Untagged { .. } => "Untagged",
            Self::RestrictionsChanged { .. } => "RestrictionsChanged",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
        }
    }

    fn version(&self) -> &'static str {
        "1.0"
    }
}
