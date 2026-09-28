//! Note fixture events: every `NoteEventBody` variant.

use std::collections::BTreeSet;

use vitni_core::enums::{NoteType, Restriction};
use vitni_core::ids::{AssertionId, HumanId, NoteId, TagId};
use vitni_core::note::{NoteEvent, NoteEventBody, NoteState};
use vitni_core::text::{MediaType, RichText};

use crate::backup_fixture::Builder;

/// Pushes one note through every variant, recording it in [`Builder::ids`].
pub(crate) fn events(builder: &mut Builder) {
    let note_id = NoteId::from_uuid(builder.uuid());
    let created_id = push_created(builder, note_id);
    push_content(builder, note_id);
    push_tags(builder, note_id);
    push_corrections(builder, note_id, created_id);
    builder.ids.note = Some(note_id);
}

fn push_created(builder: &mut Builder, note_id: NoteId) -> AssertionId {
    let meta = builder.meta();
    let assertion_id = meta.assertion_id;
    builder.push::<NoteState>(
        note_id,
        NoteEvent::new(
            &meta,
            NoteEventBody::NoteCreated {
                note_id,
                human_id: HumanId::new("N0001"),
            },
        ),
    );
    assertion_id
}

fn push_content(builder: &mut Builder, note_id: NoteId) {
    let bodies = [
        NoteEventBody::NoteTypeSet {
            note_id,
            note_type: NoteType::Research,
        },
        NoteEventBody::RichTextSet {
            note_id,
            text: RichText {
                text: "Ingrid's emigration record cites a farm called Haugen in Vågå.".to_owned(),
                media_type: MediaType::Markdown,
                language: None,
                translator: None,
                translations: Vec::new(),
            },
        },
        NoteEventBody::RestrictionsChanged {
            note_id,
            restrictions: BTreeSet::from([Restriction::Confidential]),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<NoteState>(note_id, NoteEvent::new(&meta, body));
    }
}

/// The tag created by the `tag` module, which always runs before `note` in the fixed build order.
#[expect(clippy::expect_used, reason = "build_log always runs tag before note")]
fn existing_tag_id(builder: &Builder) -> TagId {
    builder.ids.tag.expect("tag exists before note")
}

fn push_tags(builder: &mut Builder, note_id: NoteId) {
    let tag_id = existing_tag_id(builder);
    let bodies = [
        NoteEventBody::Tagged { note_id, tag_id },
        NoteEventBody::Untagged { note_id, tag_id },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<NoteState>(note_id, NoteEvent::new(&meta, body));
    }
}

fn push_corrections(builder: &mut Builder, note_id: NoteId, target: AssertionId) {
    let bodies = [
        NoteEventBody::AssertionRetracted { note_id, target },
        NoteEventBody::AssertionSuperseded { note_id, target },
        NoteEventBody::HumanIdChanged {
            note_id,
            human_id: HumanId::new("N0099"),
            old_human_id: HumanId::new("N0001"),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<NoteState>(note_id, NoteEvent::new(&meta, body));
    }
}
