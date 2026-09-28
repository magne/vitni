//! Person fixture events: every `PersonEventBody` variant.
//!
//! Four persons: `Ingrid` (the hero, exercising every variant), `Ola` (her spouse, also the
//! `family`/`dna_match` partner), `Kari` (their child, for `family`), and a duplicate `Ingrid`
//! record later merged into the hero via `PersonsMerged`.

use std::collections::BTreeSet;

use vitni_core::age::Age;
use vitni_core::enums::{AssociationRole, EvidenceLevel, FactType, ParticipantRole, Restriction, Sex};
use vitni_core::fact::Fact;
use vitni_core::ids::{AssertionId, CitationId, EventId, HumanId, MediaId, NoteId, PersonId, PlaceId, TagId};
use vitni_core::name::{LanguageTag, NameType, PersonName, Surname};
use vitni_core::person::{PersonEvent, PersonEventBody, PersonState};
use vitni_core::text::{Attribute, ExternalId, MediaRef};

use crate::backup_fixture::Builder;

/// Pushes four persons through every variant, recording them in [`Builder::ids`].
pub(crate) fn events(builder: &mut Builder) {
    let ingrid = PersonId::from_uuid(builder.uuid());
    let ola = PersonId::from_uuid(builder.uuid());
    let kari = PersonId::from_uuid(builder.uuid());
    let duplicate = PersonId::from_uuid(builder.uuid());
    let event_id = existing_or_new_event_id(builder);

    let created_id = push_created(builder, ingrid);
    push_claims(builder, ingrid, ola, event_id, existing_place_id(builder));
    push_attachments(
        builder,
        ingrid,
        existing_citation_id(builder),
        media_id(builder),
        existing_note_id(builder),
        existing_tag_id(builder),
    );
    push_corrections(builder, ingrid, duplicate, created_id);

    push_minimal(builder, ola, "I0002", "Ola");
    push_minimal(builder, kari, "I0003", "Kari");
    push_minimal(builder, duplicate, "I0004", "Ingrid");

    builder.ids.persons = vec![ingrid, ola, kari, duplicate];
}

/// The event aggregate's id — minted here since `person` runs before `event` in the fixed build
/// order; the `event` module reuses this same id when it later creates the aggregate.
fn existing_or_new_event_id(builder: &mut Builder) -> EventId {
    if let Some(id) = builder.ids.event {
        return id;
    }
    let id = EventId::from_uuid(builder.uuid());
    builder.ids.event = Some(id);
    id
}

fn push_created(builder: &mut Builder, person_id: PersonId) -> AssertionId {
    let meta = builder.meta();
    let assertion_id = meta.assertion_id;
    builder.push::<PersonState>(
        person_id,
        PersonEvent::new(
            &meta,
            PersonEventBody::PersonCreated {
                person_id,
                human_id: HumanId::new("I0001"),
                evidence_level: EvidenceLevel::Conclusion,
            },
        ),
    );
    assertion_id
}

fn ingrid_name() -> PersonName {
    PersonName {
        name_type: NameType::BirthName,
        given: Some("Ingrid Kristine".to_owned()),
        surnames: vec![Surname {
            prefix: None,
            surname: "Haugen".to_owned(),
            primary: true,
            connector: None,
        }],
        suffix: None,
        title: None,
        nickname: None,
        call_name: Some("Ingrid".to_owned()),
        date: None,
        language: Some(LanguageTag::new("nb")),
        transliterations: Vec::new(),
    }
}

fn push_claims(builder: &mut Builder, ingrid: PersonId, ola: PersonId, event_id: EventId, place_id: PlaceId) {
    let bodies = [
        PersonEventBody::NameAsserted {
            person_id: ingrid,
            name: ingrid_name(),
        },
        PersonEventBody::SexAsserted {
            person_id: ingrid,
            sex: Sex::Female,
        },
        PersonEventBody::FactAsserted {
            person_id: ingrid,
            fact: Fact {
                fact_type: FactType::Occupation,
                date: None,
                place_id: Some(place_id),
                value: Some("Farmer".to_owned()),
            },
        },
        PersonEventBody::ParticipationAsserted {
            person_id: ingrid,
            event_id,
            role: ParticipantRole::Witness,
            age: Some(Age {
                years: Some(37),
                ..Age::default()
            }),
            attributes: vec![Attribute {
                attribute_type: "Standing".to_owned(),
                value: "Godmother".to_owned(),
            }],
            notes: Vec::new(),
        },
        PersonEventBody::AssociationAsserted {
            person_id: ingrid,
            other: ola,
            role: AssociationRole::Spouse,
        },
        PersonEventBody::ExternalIdAdded {
            person_id: ingrid,
            external_id: ExternalId {
                authority: "Digitalarkivet".to_owned(),
                value: "pf01052041003123".to_owned(),
                kind: None,
                url: None,
            },
        },
        PersonEventBody::RestrictionsChanged {
            person_id: ingrid,
            restrictions: BTreeSet::from([Restriction::Privacy]),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<PersonState>(ingrid, PersonEvent::new(&meta, body));
    }
}

/// The citation created by the `citation` module, which always runs before `person`.
#[expect(clippy::expect_used, reason = "build_log always runs citation before person")]
fn existing_citation_id(builder: &Builder) -> CitationId {
    builder.ids.citation.expect("citation exists before person")
}

/// The place created by the `place` module, which always runs before `person`.
#[expect(clippy::expect_used, reason = "build_log always runs place before person")]
fn existing_place_id(builder: &Builder) -> PlaceId {
    builder.ids.place.expect("place exists before person")
}

/// The note created by the `note` module, which always runs before `person`.
#[expect(clippy::expect_used, reason = "build_log always runs note before person")]
fn existing_note_id(builder: &Builder) -> NoteId {
    builder.ids.note.expect("note exists before person")
}

/// The tag created by the `tag` module, which always runs before `person`.
#[expect(clippy::expect_used, reason = "build_log always runs tag before person")]
fn existing_tag_id(builder: &Builder) -> TagId {
    builder.ids.tag.expect("tag exists before person")
}

/// The media aggregate's id, already minted by an earlier module (`source`, `citation`, or
/// `place`) since all three run before `person`.
#[expect(
    clippy::expect_used,
    reason = "an earlier module always mints the media id before person"
)]
fn media_id(builder: &Builder) -> MediaId {
    builder.ids.media.expect("media id minted before person")
}

fn push_attachments(
    builder: &mut Builder,
    ingrid: PersonId,
    citation_id: CitationId,
    media_id: MediaId,
    note_id: NoteId,
    tag_id: TagId,
) {
    let bodies = [
        PersonEventBody::MediaAttached {
            person_id: ingrid,
            media: MediaRef {
                media_id,
                crop: None,
                caption: Some("Ingrid at the farm, c. 1889".to_owned()),
                citations: Vec::new(),
            },
        },
        PersonEventBody::NoteAttached {
            person_id: ingrid,
            note_id,
        },
        PersonEventBody::CitationAdded {
            person_id: ingrid,
            citation_id,
        },
        PersonEventBody::Tagged {
            person_id: ingrid,
            tag_id,
        },
        PersonEventBody::Untagged {
            person_id: ingrid,
            tag_id,
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<PersonState>(ingrid, PersonEvent::new(&meta, body));
    }
}

fn push_corrections(builder: &mut Builder, ingrid: PersonId, duplicate: PersonId, target: AssertionId) {
    let bodies = [
        PersonEventBody::HumanIdChanged {
            person_id: ingrid,
            human_id: HumanId::new("I0099"),
            old_human_id: HumanId::new("I0001"),
        },
        PersonEventBody::AssertionRetracted {
            person_id: ingrid,
            target,
        },
        PersonEventBody::AssertionSuperseded {
            person_id: ingrid,
            target,
        },
        PersonEventBody::PersonsMerged {
            surviving: ingrid,
            merged: duplicate,
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<PersonState>(ingrid, PersonEvent::new(&meta, body));
    }
}

/// A minimal persona: just enough to exist as a `family`/`dna_test`/`PersonsMerged` reference.
fn push_minimal(builder: &mut Builder, person_id: PersonId, human_id: &str, given: &str) {
    let meta = builder.meta();
    builder.push::<PersonState>(
        person_id,
        PersonEvent::new(
            &meta,
            PersonEventBody::PersonCreated {
                person_id,
                human_id: HumanId::new(human_id),
                evidence_level: EvidenceLevel::Persona,
            },
        ),
    );
    let meta = builder.meta();
    builder.push::<PersonState>(
        person_id,
        PersonEvent::new(
            &meta,
            PersonEventBody::NameAsserted {
                person_id,
                name: PersonName {
                    name_type: NameType::BirthName,
                    given: Some(given.to_owned()),
                    surnames: vec![Surname {
                        prefix: None,
                        surname: "Haugen".to_owned(),
                        primary: true,
                        connector: None,
                    }],
                    suffix: None,
                    title: None,
                    nickname: None,
                    call_name: None,
                    date: None,
                    language: Some(LanguageTag::new("nb")),
                    transliterations: Vec::new(),
                },
            },
        ),
    );
}
