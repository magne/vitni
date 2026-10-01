//! Source fixture events: every `SourceEventBody` variant.

use std::collections::BTreeSet;

use vitni_core::enums::{Restriction, SourceMediaType};
use vitni_core::ids::{AssertionId, HumanId, MediaId, NoteId, RepositoryId, SourceId, TagId};
use vitni_core::repo_ref::RepoRef;
use vitni_core::source::{SourceEvent, SourceEventBody, SourceState};
use vitni_core::text::{Attribute, MediaRef};

use crate::backup_fixture::Builder;

/// Pushes one source through every variant, recording it in [`Builder::ids`].
pub(crate) fn events(builder: &mut Builder) {
    let source_id = SourceId::from_uuid(builder.uuid());
    let created_id = push_created(builder, source_id);
    push_bibliographic(builder, source_id, existing_repository_id(builder));
    let media_id = media_id(builder);
    push_attachments(
        builder,
        source_id,
        media_id,
        existing_note_id(builder),
        existing_tag_id(builder),
    );
    push_corrections(builder, source_id, created_id);
    builder.ids.source = Some(source_id);
}

fn push_created(builder: &mut Builder, source_id: SourceId) -> AssertionId {
    let meta = builder.meta();
    let assertion_id = meta.assertion_id;
    builder.push::<SourceState>(
        source_id,
        SourceEvent::new(
            &meta,
            SourceEventBody::SourceCreated {
                source_id,
                human_id: HumanId::new("S0001"),
            },
        ),
    );
    assertion_id
}

fn push_bibliographic(builder: &mut Builder, source_id: SourceId, repository_id: RepositoryId) {
    let bodies = [
        SourceEventBody::TitleSet {
            source_id,
            title: "Vågå, Oppland: Ministerialbok".to_owned(),
        },
        SourceEventBody::AuthorSet {
            source_id,
            author: "Vågå prestegjeld".to_owned(),
        },
        SourceEventBody::PubInfoSet {
            source_id,
            pub_info: "Vågå kirkebok 1850-1870".to_owned(),
        },
        SourceEventBody::AbbrevSet {
            source_id,
            abbrev: "Vågå MB".to_owned(),
        },
        SourceEventBody::RepositoryLinked {
            source_id,
            repo_ref: RepoRef {
                repository_id,
                call_number: Some("SAB/A-0001".to_owned()),
                media_type: SourceMediaType::Film,
            },
        },
        SourceEventBody::AttributeAdded {
            source_id,
            attribute: Attribute {
                attribute_type: "Language".to_owned(),
                value: "Norwegian".to_owned(),
            },
        },
        SourceEventBody::RestrictionsChanged {
            source_id,
            restrictions: BTreeSet::from([Restriction::Locked]),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<SourceState>(source_id, SourceEvent::new(&meta, body));
    }
}

/// The media aggregate's id — minted here since `source` runs before `media` in the fixed build
/// order; the `media` module reuses this same id when it later creates the aggregate.
fn media_id(builder: &mut Builder) -> MediaId {
    if let Some(id) = builder.ids.media {
        return id;
    }
    let id = MediaId::from_uuid(builder.uuid());
    builder.ids.media = Some(id);
    id
}

/// The repository created by the `repository` module, which always runs before `source`.
#[expect(clippy::expect_used, reason = "build_log always runs repository before source")]
fn existing_repository_id(builder: &Builder) -> RepositoryId {
    builder.ids.repository.expect("repository exists before source")
}

/// The note created by the `note` module, which always runs before `source`.
#[expect(clippy::expect_used, reason = "build_log always runs note before source")]
fn existing_note_id(builder: &Builder) -> NoteId {
    builder.ids.note.expect("note exists before source")
}

/// The tag created by the `tag` module, which always runs before `source`.
#[expect(clippy::expect_used, reason = "build_log always runs tag before source")]
fn existing_tag_id(builder: &Builder) -> TagId {
    builder.ids.tag.expect("tag exists before source")
}

fn push_attachments(builder: &mut Builder, source_id: SourceId, media_id: MediaId, note_id: NoteId, tag_id: TagId) {
    let bodies = [
        SourceEventBody::MediaAttached {
            source_id,
            media: MediaRef {
                media_id,
                crop: None,
                caption: Some("Front page of the register".to_owned()),
                citations: Vec::new(),
            },
        },
        SourceEventBody::NoteAttached { source_id, note_id },
        SourceEventBody::Tagged { source_id, tag_id },
        SourceEventBody::Untagged { source_id, tag_id },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<SourceState>(source_id, SourceEvent::new(&meta, body));
    }
}

/// The source's corrections and identity decisions: merged with one duplicate copy and distinguished
/// from another, both minted here — the projection needs neither to exist.
fn push_corrections(builder: &mut Builder, source_id: SourceId, target: AssertionId) {
    let (duplicate, other) = (SourceId::from_uuid(builder.uuid()), SourceId::from_uuid(builder.uuid()));
    let bodies = [
        SourceEventBody::AssertionRetracted { source_id, target },
        SourceEventBody::AssertionSuperseded { source_id, target },
        SourceEventBody::HumanIdChanged {
            source_id,
            human_id: HumanId::new("S0099"),
            old_human_id: HumanId::new("S0001"),
        },
        SourceEventBody::SourcesMerged {
            surviving: source_id,
            merged: duplicate,
            assessment: None,
        },
        SourceEventBody::SourcesDistinguished {
            source: source_id,
            other,
            assessment: None,
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<SourceState>(source_id, SourceEvent::new(&meta, body));
    }
}
