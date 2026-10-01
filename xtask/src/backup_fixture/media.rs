//! Media fixture events: every `MediaEventBody` variant.

use std::collections::BTreeSet;

use vitni_core::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use vitni_core::enums::Restriction;
use vitni_core::ids::{AssertionId, CitationId, HumanId, MediaId, NoteId, TagId};
use vitni_core::media::{MediaEvent, MediaEventBody, MediaState};
use vitni_core::media_path::{MediaPath, workspace_media_path};
use vitni_core::text::Attribute;

use crate::backup_fixture::{Builder, MEDIA_FILE};

/// The sha256 digest of [`crate::backup_fixture::MEDIA_BYTES`] (computed once, hardcoded — xtask
/// carries no `sha2` dependency).
const MEDIA_CHECKSUM: &str = "sha256:373419a08dc5546bf48c9271d5dbda5d445752acb2a5715f0fc39b7a572bb6ea";

/// Pushes one media object through every variant, recording it in [`Builder::ids`].
pub(crate) fn events(builder: &mut Builder) {
    let media_id = existing_or_new_media_id(builder);
    let created_id = push_created(builder, media_id);
    push_location(builder, media_id);
    push_attachments(
        builder,
        media_id,
        existing_citation_id(builder),
        existing_note_id(builder),
        existing_tag_id(builder),
    );
    push_corrections(builder, media_id, created_id);
    builder.ids.media = Some(media_id);
}

/// Reuses the id an earlier module (`source`, `citation`, or `place`) already minted for this
/// aggregate, or mints a fresh one if none of them needed it.
fn existing_or_new_media_id(builder: &mut Builder) -> MediaId {
    match builder.ids.media {
        Some(id) => id,
        None => MediaId::from_uuid(builder.uuid()),
    }
}

fn push_created(builder: &mut Builder, media_id: MediaId) -> AssertionId {
    let meta = builder.meta();
    let assertion_id = meta.assertion_id;
    builder.push::<MediaState>(
        media_id,
        MediaEvent::new(
            &meta,
            MediaEventBody::MediaCreated {
                media_id,
                human_id: HumanId::new("O0001"),
            },
        ),
    );
    assertion_id
}

fn letter_date() -> GenealogicalDate {
    GenealogicalDate {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(1889),
            month: Some(9),
            day: Some(1),
        })),
        time: None,
        new_year_begins: None,
        sort_value: 18_890_901,
        original_text: Some("1 September 1889".to_owned()),
    }
}

fn push_location(builder: &mut Builder, media_id: MediaId) {
    let bodies = [
        MediaEventBody::PathSet {
            media_id,
            path: MediaPath::File(workspace_media_path(MEDIA_FILE)),
        },
        MediaEventBody::ChecksumSet {
            media_id,
            checksum: MEDIA_CHECKSUM.to_owned(),
        },
        MediaEventBody::MimeSet {
            media_id,
            mime: "text/plain".to_owned(),
        },
        MediaEventBody::DateAsserted {
            media_id,
            date: letter_date(),
        },
        MediaEventBody::AttributeAdded {
            media_id,
            attribute: Attribute {
                attribute_type: "Condition".to_owned(),
                value: "Fragile".to_owned(),
            },
        },
        MediaEventBody::RestrictionsChanged {
            media_id,
            restrictions: BTreeSet::from([Restriction::Confidential]),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<MediaState>(media_id, MediaEvent::new(&meta, body));
    }
}

/// The citation created by the `citation` module, which always runs before `media`.
#[expect(clippy::expect_used, reason = "build_log always runs citation before media")]
fn existing_citation_id(builder: &Builder) -> CitationId {
    builder.ids.citation.expect("citation exists before media")
}

/// The note created by the `note` module, which always runs before `media`.
#[expect(clippy::expect_used, reason = "build_log always runs note before media")]
fn existing_note_id(builder: &Builder) -> NoteId {
    builder.ids.note.expect("note exists before media")
}

/// The tag created by the `tag` module, which always runs before `media`.
#[expect(clippy::expect_used, reason = "build_log always runs tag before media")]
fn existing_tag_id(builder: &Builder) -> TagId {
    builder.ids.tag.expect("tag exists before media")
}

fn push_attachments(builder: &mut Builder, media_id: MediaId, citation_id: CitationId, note_id: NoteId, tag_id: TagId) {
    let bodies = [
        MediaEventBody::CitationAdded { media_id, citation_id },
        MediaEventBody::NoteAttached { media_id, note_id },
        MediaEventBody::Tagged { media_id, tag_id },
        MediaEventBody::Untagged { media_id, tag_id },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<MediaState>(media_id, MediaEvent::new(&meta, body));
    }
}

/// The media's corrections and identity decisions: merged with one duplicate copy and distinguished
/// from another, both minted here — the projection needs neither to exist.
fn push_corrections(builder: &mut Builder, media_id: MediaId, target: AssertionId) {
    let (duplicate, other) = (MediaId::from_uuid(builder.uuid()), MediaId::from_uuid(builder.uuid()));
    let bodies = [
        MediaEventBody::AssertionRetracted { media_id, target },
        MediaEventBody::AssertionSuperseded { media_id, target },
        MediaEventBody::HumanIdChanged {
            media_id,
            human_id: HumanId::new("O0099"),
            old_human_id: HumanId::new("O0001"),
        },
        MediaEventBody::MediaMerged {
            surviving: media_id,
            merged: duplicate,
            assessment: None,
        },
        MediaEventBody::MediaDistinguished {
            media: media_id,
            other,
            assessment: None,
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<MediaState>(media_id, MediaEvent::new(&meta, body));
    }
}
