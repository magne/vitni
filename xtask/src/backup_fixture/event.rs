//! Event fixture events: every `EventEventBody` variant.

use std::collections::BTreeSet;

use vitni_core::address::Address;
use vitni_core::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use vitni_core::enums::{EventType, Restriction};
use vitni_core::event::{EventEvent, EventEventBody, EventState};
use vitni_core::ids::{AssertionId, CitationId, EventId, HumanId, MediaId, NoteId, PlaceId, TagId};
use vitni_core::text::MediaRef;

use crate::backup_fixture::Builder;

/// Pushes one event through every variant, recording it in [`Builder::ids`].
pub(crate) fn events(builder: &mut Builder) {
    let event_id = existing_event_id(builder);
    let created_id = push_created(builder, event_id);
    push_details(builder, event_id, existing_place_id(builder));
    push_attachments(
        builder,
        event_id,
        existing_citation_id(builder),
        existing_media_id(builder),
        existing_note_id(builder),
        existing_tag_id(builder),
    );
    push_corrections(builder, event_id, created_id);
    builder.ids.event = Some(event_id);
}

/// Reuses the id `person` already minted for this aggregate (needed early for
/// `ParticipationAsserted`).
#[expect(
    clippy::expect_used,
    reason = "build_log always runs person before event, minting this id"
)]
fn existing_event_id(builder: &Builder) -> EventId {
    builder.ids.event.expect("person mints the event id before event runs")
}

fn push_created(builder: &mut Builder, event_id: EventId) -> AssertionId {
    let meta = builder.meta();
    let assertion_id = meta.assertion_id;
    builder.push::<EventState>(
        event_id,
        EventEvent::new(
            &meta,
            EventEventBody::EventCreated {
                event_id,
                human_id: HumanId::new("E0001"),
                event_type: EventType::Baptism,
            },
        ),
    );
    assertion_id
}

fn baptism_date() -> GenealogicalDate {
    GenealogicalDate {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(1852),
            month: Some(4),
            day: Some(11),
        })),
        time: None,
        new_year_begins: None,
        sort_value: 18_520_411,
        original_text: Some("11 April 1852".to_owned()),
    }
}

fn push_details(builder: &mut Builder, event_id: EventId, place_id: PlaceId) {
    let bodies = [
        EventEventBody::EventTypeSet {
            event_id,
            event_type: EventType::Baptism,
        },
        EventEventBody::DateAsserted {
            event_id,
            date: baptism_date(),
        },
        EventEventBody::DescriptionSet {
            event_id,
            description: "Baptism at Vågå church".to_owned(),
        },
        EventEventBody::PlaceLinked { event_id, place_id },
        EventEventBody::AddressAdded {
            event_id,
            address: Address {
                locality: Some("Vågå".to_owned()),
                country: Some("Norway".to_owned()),
                ..Address::default()
            },
        },
        EventEventBody::RestrictionsChanged {
            event_id,
            restrictions: BTreeSet::from([Restriction::Confidential]),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<EventState>(event_id, EventEvent::new(&meta, body));
    }
}

/// The citation created by the `citation` module, which always runs before `event`.
#[expect(clippy::expect_used, reason = "build_log always runs citation before event")]
fn existing_citation_id(builder: &Builder) -> CitationId {
    builder.ids.citation.expect("citation exists before event")
}

/// The place created by the `place` module, which always runs before `event`.
#[expect(clippy::expect_used, reason = "build_log always runs place before event")]
fn existing_place_id(builder: &Builder) -> PlaceId {
    builder.ids.place.expect("place exists before event")
}

/// The media created by the `media` module, which always runs before `event`.
#[expect(clippy::expect_used, reason = "build_log always runs media before event")]
fn existing_media_id(builder: &Builder) -> MediaId {
    builder.ids.media.expect("media exists before event")
}

/// The note created by the `note` module, which always runs before `event`.
#[expect(clippy::expect_used, reason = "build_log always runs note before event")]
fn existing_note_id(builder: &Builder) -> NoteId {
    builder.ids.note.expect("note exists before event")
}

/// The tag created by the `tag` module, which always runs before `event`.
#[expect(clippy::expect_used, reason = "build_log always runs tag before event")]
fn existing_tag_id(builder: &Builder) -> TagId {
    builder.ids.tag.expect("tag exists before event")
}

fn push_attachments(
    builder: &mut Builder,
    event_id: EventId,
    citation_id: CitationId,
    media_id: MediaId,
    note_id: NoteId,
    tag_id: TagId,
) {
    let bodies = [
        EventEventBody::CitationAdded { event_id, citation_id },
        EventEventBody::MediaAttached {
            event_id,
            media: MediaRef {
                media_id,
                crop: None,
                caption: Some("The church register entry".to_owned()),
                citations: Vec::new(),
            },
        },
        EventEventBody::NoteAttached { event_id, note_id },
        EventEventBody::Tagged { event_id, tag_id },
        EventEventBody::Untagged { event_id, tag_id },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<EventState>(event_id, EventEvent::new(&meta, body));
    }
}

fn push_corrections(builder: &mut Builder, event_id: EventId, target: AssertionId) {
    let bodies = [
        EventEventBody::AssertionRetracted { event_id, target },
        EventEventBody::AssertionSuperseded { event_id, target },
        EventEventBody::HumanIdChanged {
            event_id,
            human_id: HumanId::new("E0099"),
            old_human_id: HumanId::new("E0001"),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<EventState>(event_id, EventEvent::new(&meta, body));
    }
}
