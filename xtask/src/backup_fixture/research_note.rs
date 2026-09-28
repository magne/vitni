//! `ResearchNote` fixture events: every `ResearchNoteEventBody` variant.

use std::collections::BTreeSet;

use vitni_core::enums::Restriction;
use vitni_core::ids::{AssertionId, EventId, FamilyId, HumanId, PersonId, PlaceId, ResearchNoteId, TagId};
use vitni_core::research_note::{ResearchNoteEvent, ResearchNoteEventBody, ResearchNoteState, SubjectRef};
use vitni_core::text::{MediaType, RichText};

use crate::backup_fixture::Builder;

/// Pushes one research note through every variant.
pub(crate) fn events(builder: &mut Builder) {
    let research_note_id = ResearchNoteId::from_uuid(builder.uuid());
    let (ingrid, other_person, family_id, event_id, place_id) = existing_subjects(builder);
    let created_id = push_created(builder, research_note_id, ingrid, family_id, event_id, place_id);
    push_subjects(builder, research_note_id, other_person);
    push_content_and_tags(builder, research_note_id, existing_tag_id(builder));
    push_corrections(builder, research_note_id, created_id);
}

/// The Person/Family/Event/Place ids `person`, `family`, `event` and `place` created, all of
/// which always run before `research_note`.
#[expect(
    clippy::expect_used,
    reason = "build_log always runs these modules before research_note"
)]
fn existing_subjects(builder: &Builder) -> (PersonId, PersonId, FamilyId, EventId, PlaceId) {
    let ingrid = *builder.ids.persons.first().expect("person creates at least one person");
    let other_person = *builder.ids.persons.get(1).expect("person creates at least two persons");
    let family_id = builder.ids.family.expect("family exists before research_note");
    let event_id = builder.ids.event.expect("event exists before research_note");
    let place_id = builder.ids.place.expect("place exists before research_note");
    (ingrid, other_person, family_id, event_id, place_id)
}

fn push_created(
    builder: &mut Builder,
    research_note_id: ResearchNoteId,
    ingrid: PersonId,
    family_id: FamilyId,
    event_id: EventId,
    place_id: PlaceId,
) -> AssertionId {
    let subjects = BTreeSet::from([
        SubjectRef::Person(ingrid),
        SubjectRef::Family(family_id),
        SubjectRef::Event(event_id),
        SubjectRef::Place(place_id),
    ]);
    let meta = builder.meta();
    let assertion_id = meta.assertion_id;
    builder.push::<ResearchNoteState>(
        research_note_id,
        ResearchNoteEvent::new(
            &meta,
            ResearchNoteEventBody::ResearchNoteCreated {
                research_note_id,
                human_id: HumanId::new("A0001"),
                subjects,
                title: Some("Is the Haugen baptism entry Ingrid's?".to_owned()),
            },
        ),
    );
    assertion_id
}

fn push_subjects(builder: &mut Builder, research_note_id: ResearchNoteId, other_person: PersonId) {
    let subject = SubjectRef::Person(other_person);
    let bodies = [
        ResearchNoteEventBody::SubjectAdded {
            research_note_id,
            subject,
        },
        ResearchNoteEventBody::SubjectRemoved {
            research_note_id,
            subject,
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<ResearchNoteState>(research_note_id, ResearchNoteEvent::new(&meta, body));
    }
}

/// The tag created by the `tag` module, which always runs before `research_note`.
#[expect(clippy::expect_used, reason = "build_log always runs tag before research_note")]
fn existing_tag_id(builder: &Builder) -> TagId {
    builder.ids.tag.expect("tag exists before research_note")
}

fn push_content_and_tags(builder: &mut Builder, research_note_id: ResearchNoteId, tag_id: TagId) {
    let bodies = [
        ResearchNoteEventBody::RichTextSet {
            research_note_id,
            body: RichText {
                text: "The register's Ingrid Haugen matches the emigration record on age and farm.".to_owned(),
                media_type: MediaType::Markdown,
                language: None,
                translator: None,
                translations: Vec::new(),
            },
        },
        ResearchNoteEventBody::Tagged {
            research_note_id,
            tag_id,
        },
        ResearchNoteEventBody::Untagged {
            research_note_id,
            tag_id,
        },
        ResearchNoteEventBody::RestrictionsChanged {
            research_note_id,
            restrictions: BTreeSet::from([Restriction::Confidential]),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<ResearchNoteState>(research_note_id, ResearchNoteEvent::new(&meta, body));
    }
}

fn push_corrections(builder: &mut Builder, research_note_id: ResearchNoteId, target: AssertionId) {
    let bodies = [
        ResearchNoteEventBody::AssertionRetracted {
            research_note_id,
            target,
        },
        ResearchNoteEventBody::AssertionSuperseded {
            research_note_id,
            target,
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<ResearchNoteState>(research_note_id, ResearchNoteEvent::new(&meta, body));
    }
}
