//! `DnaMatch` fixture events: every `DnaMatchEventBody` variant.
//!
//! A match needs two `DnaTest` aggregates; `dna_test` created the first (`builder.ids.dna_test`),
//! so this module creates the second itself — a minimal test for Ola, the fixture's other partner.

use std::collections::BTreeSet;

use vitni_core::dna::{Centimorgans, ChromosomeSide, DnaProvider, DnaSegment, PercentShared, SharedAncestor};
use vitni_core::dna_match::{DnaMatchEvent, DnaMatchEventBody, DnaMatchState};
use vitni_core::dna_test::{DnaTestEvent, DnaTestEventBody, DnaTestState};
use vitni_core::enums::Restriction;
use vitni_core::ids::{AssertionId, DnaMatchId, DnaTestId, HumanId, NoteId, PersonId, TagId};

use crate::backup_fixture::Builder;

/// Pushes one DNA match through every variant, recording it in [`Builder::ids`].
pub(crate) fn events(builder: &mut Builder) {
    let test_a = existing_dna_test_id(builder);
    let test_b = push_second_test(builder, existing_second_person_id(builder));
    let dna_match_id = DnaMatchId::from_uuid(builder.uuid());
    let created_id = push_created(builder, dna_match_id, test_a, test_b);
    push_evidence(builder, dna_match_id, existing_first_person_id(builder));
    push_attachments(
        builder,
        dna_match_id,
        existing_note_id(builder),
        existing_tag_id(builder),
    );
    push_corrections(builder, dna_match_id, created_id);
    builder.ids.dna_match = Some(dna_match_id);
}

/// The DNA test created by the `dna_test` module, which always runs before `dna_match`.
#[expect(clippy::expect_used, reason = "build_log always runs dna_test before dna_match")]
fn existing_dna_test_id(builder: &Builder) -> DnaTestId {
    builder.ids.dna_test.expect("a dna_test exists before dna_match")
}

/// The first person `person` created (Ingrid), the match's inferred shared ancestor's descendant.
#[expect(clippy::expect_used, reason = "build_log always runs person before dna_match")]
fn existing_first_person_id(builder: &Builder) -> PersonId {
    *builder.ids.persons.first().expect("person creates at least one person")
}

/// The second person `person` created (Ola), who takes the fixture's second DNA test.
#[expect(clippy::expect_used, reason = "build_log always runs person before dna_match")]
fn existing_second_person_id(builder: &Builder) -> PersonId {
    *builder.ids.persons.get(1).expect("person creates at least two persons")
}

/// A second, minimal `DnaTest` aggregate — not exposed through [`Builder::ids`], since only
/// `dna_match` needs it.
fn push_second_test(builder: &mut Builder, person_id: PersonId) -> DnaTestId {
    let dna_test_id = DnaTestId::from_uuid(builder.uuid());
    let meta = builder.meta();
    builder.push::<DnaTestState>(
        dna_test_id,
        DnaTestEvent::new(
            &meta,
            DnaTestEventBody::DnaTestCreated {
                dna_test_id,
                human_id: HumanId::new("D0002"),
                person_id,
            },
        ),
    );
    dna_test_id
}

fn push_created(builder: &mut Builder, dna_match_id: DnaMatchId, test_a: DnaTestId, test_b: DnaTestId) -> AssertionId {
    let meta = builder.meta();
    let assertion_id = meta.assertion_id;
    builder.push::<DnaMatchState>(
        dna_match_id,
        DnaMatchEvent::new(
            &meta,
            DnaMatchEventBody::DnaMatchObserved {
                dna_match_id,
                human_id: HumanId::new("X0001"),
                test_a,
                test_b,
                provider: DnaProvider::AncestryDna,
                shared_cm: Centimorgans::from_hundredths(85_050),
                percent_shared: Some(PercentShared::from_ten_thousandths(123_456)),
                segment_count: 12,
                largest_segment_cm: Centimorgans::from_hundredths(3_500),
                predicted_relationship: Some("2nd cousin".to_owned()),
            },
        ),
    );
    assertion_id
}

fn push_evidence(builder: &mut Builder, dna_match_id: DnaMatchId, ancestor_person_id: PersonId) {
    let bodies = [
        DnaMatchEventBody::SegmentAdded {
            dna_match_id,
            segment: DnaSegment {
                chromosome: "7".to_owned(),
                start: 1_000_000,
                end: 5_000_000,
                centimorgans: Centimorgans::from_hundredths(1_234),
                snps: Some(2_500),
                side: ChromosomeSide::Maternal,
            },
        },
        DnaMatchEventBody::SharedAncestorAsserted {
            dna_match_id,
            ancestor: SharedAncestor {
                ancestor_person_id: Some(ancestor_person_id),
                note: Some("Possibly Ingrid's great-grandmother".to_owned()),
            },
        },
        DnaMatchEventBody::MatchConfirmed { dna_match_id },
        DnaMatchEventBody::RestrictionsChanged {
            dna_match_id,
            restrictions: BTreeSet::from([Restriction::Confidential]),
        },
        DnaMatchEventBody::MatchRejected { dna_match_id },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<DnaMatchState>(dna_match_id, DnaMatchEvent::new(&meta, body));
    }
}

/// The note created by the `note` module, which always runs before `dna_match`.
#[expect(clippy::expect_used, reason = "build_log always runs note before dna_match")]
fn existing_note_id(builder: &Builder) -> NoteId {
    builder.ids.note.expect("note exists before dna_match")
}

/// The tag created by the `tag` module, which always runs before `dna_match`.
#[expect(clippy::expect_used, reason = "build_log always runs tag before dna_match")]
fn existing_tag_id(builder: &Builder) -> TagId {
    builder.ids.tag.expect("tag exists before dna_match")
}

fn push_attachments(builder: &mut Builder, dna_match_id: DnaMatchId, note_id: NoteId, tag_id: TagId) {
    let bodies = [
        DnaMatchEventBody::NoteAttached { dna_match_id, note_id },
        DnaMatchEventBody::Tagged { dna_match_id, tag_id },
        DnaMatchEventBody::Untagged { dna_match_id, tag_id },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<DnaMatchState>(dna_match_id, DnaMatchEvent::new(&meta, body));
    }
}

fn push_corrections(builder: &mut Builder, dna_match_id: DnaMatchId, target: AssertionId) {
    let bodies = [
        DnaMatchEventBody::AssertionRetracted { dna_match_id, target },
        DnaMatchEventBody::AssertionSuperseded { dna_match_id, target },
        DnaMatchEventBody::HumanIdChanged {
            dna_match_id,
            human_id: HumanId::new("X0099"),
            old_human_id: HumanId::new("X0001"),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<DnaMatchState>(dna_match_id, DnaMatchEvent::new(&meta, body));
    }
}
