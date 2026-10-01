//! Place fixture events: every `PlaceEventBody` variant.
//!
//! Two places: `Haugen` (a farm, the fixture's canonical place) enclosed by `Vågå` (a municipality),
//! which also stands in for the `SuccessionAsserted` merger target.

use std::collections::BTreeSet;
use std::str::FromStr;

use vitni_core::enums::{PlaceType, Restriction, SuccessionKind};
use vitni_core::geo::{GeoCoordinates, Microdegrees, PlaceGeometry};
use vitni_core::ids::{AssertionId, CitationId, HumanId, MediaId, NoteId, PlaceId, TagId};
use vitni_core::name::LanguageTag;
use vitni_core::place::{PlaceEvent, PlaceEventBody, PlaceState};
use vitni_core::place_name::PlaceName;
use vitni_core::place_ref::PlaceRef;
use vitni_core::text::MediaRef;

use crate::backup_fixture::Builder;

/// Pushes two places through every variant, recording the farm in [`Builder::ids`].
pub(crate) fn events(builder: &mut Builder) {
    let farm_id = PlaceId::from_uuid(builder.uuid());
    let municipality_id = PlaceId::from_uuid(builder.uuid());
    push_municipality(builder, municipality_id);
    let created_id = push_created(builder, farm_id);
    push_names_and_shape(builder, farm_id, municipality_id);
    let media_id = media_id(builder);
    push_attachments(
        builder,
        farm_id,
        existing_citation_id(builder),
        media_id,
        existing_note_id(builder),
        existing_tag_id(builder),
    );
    push_corrections(builder, farm_id, created_id);
    builder.ids.place = Some(farm_id);
}

/// The municipality's own minimal history: just enough to exist as an enclosing/succession target.
fn push_municipality(builder: &mut Builder, municipality_id: PlaceId) {
    let meta = builder.meta();
    builder.push::<PlaceState>(
        municipality_id,
        PlaceEvent::new(
            &meta,
            PlaceEventBody::PlaceCreated {
                place_id: municipality_id,
                human_id: HumanId::new("P0002"),
                place_type: PlaceType::Municipality,
            },
        ),
    );
    let meta = builder.meta();
    builder.push::<PlaceState>(
        municipality_id,
        PlaceEvent::new(
            &meta,
            PlaceEventBody::NameAsserted {
                place_id: municipality_id,
                name: PlaceName {
                    text: "Vågå".to_owned(),
                    language: Some(LanguageTag::new("nb")),
                    date: None,
                },
            },
        ),
    );
}

fn push_created(builder: &mut Builder, farm_id: PlaceId) -> AssertionId {
    let meta = builder.meta();
    let assertion_id = meta.assertion_id;
    builder.push::<PlaceState>(
        farm_id,
        PlaceEvent::new(
            &meta,
            PlaceEventBody::PlaceCreated {
                place_id: farm_id,
                human_id: HumanId::new("P0001"),
                place_type: PlaceType::Farm,
            },
        ),
    );
    assertion_id
}

fn point(lat: &str, lon: &str) -> GeoCoordinates {
    GeoCoordinates {
        latitude: Microdegrees::from_str(lat).unwrap_or(Microdegrees::from_microdegrees(0)),
        longitude: Microdegrees::from_str(lon).unwrap_or(Microdegrees::from_microdegrees(0)),
    }
}

fn push_names_and_shape(builder: &mut Builder, farm_id: PlaceId, municipality_id: PlaceId) {
    let bodies = [
        PlaceEventBody::PlaceTypeSet {
            place_id: farm_id,
            place_type: PlaceType::Farm,
        },
        PlaceEventBody::NameAsserted {
            place_id: farm_id,
            name: PlaceName {
                text: "Haugen".to_owned(),
                language: Some(LanguageTag::new("nb")),
                date: None,
            },
        },
        PlaceEventBody::EnclosedByAsserted {
            place_id: farm_id,
            enclosed_by: PlaceRef {
                place_id: municipality_id,
                date: None,
            },
        },
        PlaceEventBody::CoordinatesAsserted {
            place_id: farm_id,
            coordinates: point("61.878", "8.85"),
        },
        PlaceEventBody::GeometryAsserted {
            place_id: farm_id,
            geometry: PlaceGeometry::Point(point("61.878", "8.85")),
            date: None,
        },
        PlaceEventBody::SuccessionAsserted {
            place_id: farm_id,
            from: vec![farm_id],
            to: vec![municipality_id],
            kind: SuccessionKind::Absorbed,
            date: None,
        },
        PlaceEventBody::CodeSet {
            place_id: farm_id,
            code: "0439".to_owned(),
        },
        PlaceEventBody::RestrictionsChanged {
            place_id: farm_id,
            restrictions: BTreeSet::from([Restriction::Confidential]),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<PlaceState>(farm_id, PlaceEvent::new(&meta, body));
    }
}

/// The media aggregate's id — minted here if `source`/`citation` did not already mint it, since
/// `place` runs before `media` in the fixed build order.
fn media_id(builder: &mut Builder) -> MediaId {
    if let Some(id) = builder.ids.media {
        return id;
    }
    let id = MediaId::from_uuid(builder.uuid());
    builder.ids.media = Some(id);
    id
}

/// The citation created by the `citation` module, which always runs before `place`.
#[expect(clippy::expect_used, reason = "build_log always runs citation before place")]
fn existing_citation_id(builder: &Builder) -> CitationId {
    builder.ids.citation.expect("citation exists before place")
}

/// The note created by the `note` module, which always runs before `place`.
#[expect(clippy::expect_used, reason = "build_log always runs note before place")]
fn existing_note_id(builder: &Builder) -> NoteId {
    builder.ids.note.expect("note exists before place")
}

/// The tag created by the `tag` module, which always runs before `place`.
#[expect(clippy::expect_used, reason = "build_log always runs tag before place")]
fn existing_tag_id(builder: &Builder) -> TagId {
    builder.ids.tag.expect("tag exists before place")
}

fn push_attachments(
    builder: &mut Builder,
    farm_id: PlaceId,
    citation_id: CitationId,
    media_id: MediaId,
    note_id: NoteId,
    tag_id: TagId,
) {
    let bodies = [
        PlaceEventBody::CitationAdded {
            place_id: farm_id,
            citation_id,
        },
        PlaceEventBody::MediaAttached {
            place_id: farm_id,
            media: MediaRef {
                media_id,
                crop: None,
                caption: Some("A photo of the farmhouse".to_owned()),
                citations: Vec::new(),
            },
        },
        PlaceEventBody::NoteAttached {
            place_id: farm_id,
            note_id,
        },
        PlaceEventBody::Tagged {
            place_id: farm_id,
            tag_id,
        },
        PlaceEventBody::Untagged {
            place_id: farm_id,
            tag_id,
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<PlaceState>(farm_id, PlaceEvent::new(&meta, body));
    }
}

/// The place's corrections and identity decisions: merged with one duplicate copy and distinguished
/// from another, both minted here — the projection needs neither to exist.
fn push_corrections(builder: &mut Builder, farm_id: PlaceId, target: AssertionId) {
    let (duplicate, other) = (PlaceId::from_uuid(builder.uuid()), PlaceId::from_uuid(builder.uuid()));
    let bodies = [
        PlaceEventBody::AssertionRetracted {
            place_id: farm_id,
            target,
        },
        PlaceEventBody::AssertionSuperseded {
            place_id: farm_id,
            target,
        },
        PlaceEventBody::HumanIdChanged {
            place_id: farm_id,
            human_id: HumanId::new("P0099"),
            old_human_id: HumanId::new("P0001"),
        },
        PlaceEventBody::PlacesMerged {
            surviving: farm_id,
            merged: duplicate,
            assessment: None,
        },
        PlaceEventBody::PlacesDistinguished {
            place: farm_id,
            other,
            assessment: None,
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<PlaceState>(farm_id, PlaceEvent::new(&meta, body));
    }
}
