//! Family fixture events: every `FamilyEventBody` variant.

use std::collections::BTreeSet;

use vitni_core::enums::{ChildParentRelationship, Restriction};
use vitni_core::family::{FamilyEvent, FamilyEventBody, FamilyState};
use vitni_core::ids::{AssertionId, CitationId, EventId, FamilyId, HumanId, MediaId, NoteId, PersonId, TagId};
use vitni_core::text::{ExternalId, MediaRef};

use crate::backup_fixture::Builder;

/// Pushes one family through every variant, recording it in [`Builder::ids`].
pub(crate) fn events(builder: &mut Builder) {
    let family_id = FamilyId::from_uuid(builder.uuid());
    let (partner_a, partner_b, child) = existing_persons(builder);
    let created_id = push_created(builder, family_id);
    push_membership(builder, family_id, partner_a, partner_b, child);
    push_links(
        builder,
        family_id,
        existing_citation_id(builder),
        existing_event_id(builder),
    );
    push_attachments(
        builder,
        family_id,
        existing_media_id(builder),
        existing_note_id(builder),
        existing_tag_id(builder),
    );
    push_corrections(builder, family_id, created_id);
    builder.ids.family = Some(family_id);
}

/// The first three persons `person` created: two partners and their child.
#[expect(
    clippy::expect_used,
    reason = "build_log always runs person before family, with 4 persons"
)]
fn existing_persons(builder: &Builder) -> (PersonId, PersonId, PersonId) {
    let persons = &builder.ids.persons;
    let partner_a = *persons.first().expect("person creates at least two persons");
    let partner_b = *persons.get(1).expect("person creates at least two persons");
    let child = *persons.get(2).expect("person creates at least three persons");
    (partner_a, partner_b, child)
}

fn push_created(builder: &mut Builder, family_id: FamilyId) -> AssertionId {
    let meta = builder.meta();
    let assertion_id = meta.assertion_id;
    builder.push::<FamilyState>(
        family_id,
        FamilyEvent::new(
            &meta,
            FamilyEventBody::FamilyCreated {
                family_id,
                human_id: HumanId::new("F0001"),
            },
        ),
    );
    assertion_id
}

fn push_membership(
    builder: &mut Builder,
    family_id: FamilyId,
    partner_a: PersonId,
    partner_b: PersonId,
    child: PersonId,
) {
    let bodies = [
        FamilyEventBody::PartnerAdded {
            family_id,
            person_id: partner_a,
        },
        FamilyEventBody::PartnerAdded {
            family_id,
            person_id: partner_b,
        },
        FamilyEventBody::ChildAdded {
            family_id,
            child_id: child,
        },
        FamilyEventBody::ChildRelationshipAsserted {
            family_id,
            child_id: child,
            parent_id: partner_a,
            relationship: ChildParentRelationship::Birth,
        },
        FamilyEventBody::ExternalIdAdded {
            family_id,
            external_id: ExternalId {
                authority: "Digitalarkivet".to_owned(),
                value: "fam-vaaga-0012".to_owned(),
                kind: None,
                url: None,
            },
        },
        FamilyEventBody::RestrictionsChanged {
            family_id,
            restrictions: BTreeSet::from([Restriction::Confidential]),
        },
        FamilyEventBody::PartnerRemoved {
            family_id,
            person_id: partner_b,
        },
        FamilyEventBody::ChildRemoved {
            family_id,
            child_id: child,
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<FamilyState>(family_id, FamilyEvent::new(&meta, body));
    }
}

/// The citation created by the `citation` module, which always runs before `family`.
#[expect(clippy::expect_used, reason = "build_log always runs citation before family")]
fn existing_citation_id(builder: &Builder) -> CitationId {
    builder.ids.citation.expect("citation exists before family")
}

/// The event created by the `event` module, which always runs before `family`.
#[expect(clippy::expect_used, reason = "build_log always runs event before family")]
fn existing_event_id(builder: &Builder) -> EventId {
    builder.ids.event.expect("event exists before family")
}

fn push_links(builder: &mut Builder, family_id: FamilyId, citation_id: CitationId, event_id: EventId) {
    let bodies = [
        FamilyEventBody::CitationAdded { family_id, citation_id },
        FamilyEventBody::FamilyEventLinked { family_id, event_id },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<FamilyState>(family_id, FamilyEvent::new(&meta, body));
    }
}

/// The media created by the `media` module, which always runs before `family`.
#[expect(clippy::expect_used, reason = "build_log always runs media before family")]
fn existing_media_id(builder: &Builder) -> MediaId {
    builder.ids.media.expect("media exists before family")
}

/// The note created by the `note` module, which always runs before `family`.
#[expect(clippy::expect_used, reason = "build_log always runs note before family")]
fn existing_note_id(builder: &Builder) -> NoteId {
    builder.ids.note.expect("note exists before family")
}

/// The tag created by the `tag` module, which always runs before `family`.
#[expect(clippy::expect_used, reason = "build_log always runs tag before family")]
fn existing_tag_id(builder: &Builder) -> TagId {
    builder.ids.tag.expect("tag exists before family")
}

fn push_attachments(builder: &mut Builder, family_id: FamilyId, media_id: MediaId, note_id: NoteId, tag_id: TagId) {
    let bodies = [
        FamilyEventBody::MediaAttached {
            family_id,
            media: MediaRef {
                media_id,
                crop: None,
                caption: Some("The family outside the farmhouse".to_owned()),
                citations: Vec::new(),
            },
        },
        FamilyEventBody::NoteAttached { family_id, note_id },
        FamilyEventBody::Tagged { family_id, tag_id },
        FamilyEventBody::Untagged { family_id, tag_id },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<FamilyState>(family_id, FamilyEvent::new(&meta, body));
    }
}

fn push_corrections(builder: &mut Builder, family_id: FamilyId, target: AssertionId) {
    let bodies = [
        FamilyEventBody::AssertionRetracted { family_id, target },
        FamilyEventBody::AssertionSuperseded { family_id, target },
        FamilyEventBody::HumanIdChanged {
            family_id,
            human_id: HumanId::new("F0099"),
            old_human_id: HumanId::new("F0001"),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<FamilyState>(family_id, FamilyEvent::new(&meta, body));
    }
}
