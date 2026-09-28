//! `DnaTest` fixture events: every `DnaTestEventBody` variant.

use std::collections::BTreeSet;

use vitni_core::dna::{DnaGenomeBuild, DnaProvider, DnaTestType};
use vitni_core::dna_test::{DnaTestEvent, DnaTestEventBody, DnaTestState};
use vitni_core::enums::Restriction;
use vitni_core::ids::{AssertionId, DnaTestId, HumanId, NoteId, PersonId, TagId};

use crate::backup_fixture::Builder;

/// Pushes one DNA test through every variant, recording it in [`Builder::ids`].
pub(crate) fn events(builder: &mut Builder) {
    let dna_test_id = DnaTestId::from_uuid(builder.uuid());
    let created_id = push_created(builder, dna_test_id, existing_person_id(builder));
    push_details(builder, dna_test_id);
    push_attachments(
        builder,
        dna_test_id,
        existing_note_id(builder),
        existing_tag_id(builder),
    );
    push_corrections(builder, dna_test_id, created_id);
    builder.ids.dna_test = Some(dna_test_id);
}

/// The first person `person` created (the fixture's hero, Ingrid).
#[expect(clippy::expect_used, reason = "build_log always runs person before dna_test")]
fn existing_person_id(builder: &Builder) -> PersonId {
    *builder.ids.persons.first().expect("person creates at least one person")
}

fn push_created(builder: &mut Builder, dna_test_id: DnaTestId, person_id: PersonId) -> AssertionId {
    let meta = builder.meta();
    let assertion_id = meta.assertion_id;
    builder.push::<DnaTestState>(
        dna_test_id,
        DnaTestEvent::new(
            &meta,
            DnaTestEventBody::DnaTestCreated {
                dna_test_id,
                human_id: HumanId::new("D0001"),
                person_id,
            },
        ),
    );
    assertion_id
}

fn push_details(builder: &mut Builder, dna_test_id: DnaTestId) {
    let bodies = [
        DnaTestEventBody::ProviderSet {
            dna_test_id,
            provider: DnaProvider::AncestryDna,
        },
        DnaTestEventBody::KitIdSet {
            dna_test_id,
            kit_id: "AY-000123".to_owned(),
        },
        DnaTestEventBody::TestTypeSet {
            dna_test_id,
            test_type: DnaTestType::Autosomal,
        },
        DnaTestEventBody::GenomeBuildSet {
            dna_test_id,
            genome_build: DnaGenomeBuild::GRCh38,
        },
        DnaTestEventBody::HaplogroupAsserted {
            dna_test_id,
            haplogroup: "H1a1".to_owned(),
        },
        DnaTestEventBody::RestrictionsChanged {
            dna_test_id,
            restrictions: BTreeSet::from([Restriction::Confidential]),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<DnaTestState>(dna_test_id, DnaTestEvent::new(&meta, body));
    }
}

/// The note created by the `note` module, which always runs before `dna_test`.
#[expect(clippy::expect_used, reason = "build_log always runs note before dna_test")]
fn existing_note_id(builder: &Builder) -> NoteId {
    builder.ids.note.expect("note exists before dna_test")
}

/// The tag created by the `tag` module, which always runs before `dna_test`.
#[expect(clippy::expect_used, reason = "build_log always runs tag before dna_test")]
fn existing_tag_id(builder: &Builder) -> TagId {
    builder.ids.tag.expect("tag exists before dna_test")
}

fn push_attachments(builder: &mut Builder, dna_test_id: DnaTestId, note_id: NoteId, tag_id: TagId) {
    let bodies = [
        DnaTestEventBody::NoteAttached { dna_test_id, note_id },
        DnaTestEventBody::Tagged { dna_test_id, tag_id },
        DnaTestEventBody::Untagged { dna_test_id, tag_id },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<DnaTestState>(dna_test_id, DnaTestEvent::new(&meta, body));
    }
}

fn push_corrections(builder: &mut Builder, dna_test_id: DnaTestId, target: AssertionId) {
    let bodies = [
        DnaTestEventBody::AssertionRetracted { dna_test_id, target },
        DnaTestEventBody::AssertionSuperseded { dna_test_id, target },
        DnaTestEventBody::HumanIdChanged {
            dna_test_id,
            human_id: HumanId::new("D0099"),
            old_human_id: HumanId::new("D0001"),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<DnaTestState>(dna_test_id, DnaTestEvent::new(&meta, body));
    }
}
