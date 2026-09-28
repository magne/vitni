//! Repository fixture events: every `RepositoryEventBody` variant.

use std::collections::BTreeSet;

use vitni_core::address::Address;
use vitni_core::enums::{RepositoryType, Restriction};
use vitni_core::ids::{AssertionId, HumanId, NoteId, RepositoryId, TagId};
use vitni_core::repository::{RepositoryEvent, RepositoryEventBody, RepositoryState};
use vitni_core::text::Url;

use crate::backup_fixture::Builder;

/// Pushes one repository through every variant, recording it in [`Builder::ids`].
pub(crate) fn events(builder: &mut Builder) {
    let repository_id = RepositoryId::from_uuid(builder.uuid());
    let created_id = push_created(builder, repository_id);
    push_details(builder, repository_id);
    push_attachments(
        builder,
        repository_id,
        existing_note_id(builder),
        existing_tag_id(builder),
    );
    push_corrections(builder, repository_id, created_id);
    builder.ids.repository = Some(repository_id);
}

fn push_created(builder: &mut Builder, repository_id: RepositoryId) -> AssertionId {
    let meta = builder.meta();
    let assertion_id = meta.assertion_id;
    builder.push::<RepositoryState>(
        repository_id,
        RepositoryEvent::new(
            &meta,
            RepositoryEventBody::RepositoryCreated {
                repository_id,
                human_id: HumanId::new("R0001"),
            },
        ),
    );
    assertion_id
}

fn push_details(builder: &mut Builder, repository_id: RepositoryId) {
    let bodies = [
        RepositoryEventBody::RepositoryTypeSet {
            repository_id,
            repository_type: RepositoryType::Archive,
        },
        RepositoryEventBody::NameSet {
            repository_id,
            name: "Statsarkivet i Bergen".to_owned(),
        },
        RepositoryEventBody::AddressAdded {
            repository_id,
            address: Address {
                lines: vec!["Årstadveien 22".to_owned()],
                locality: Some("Bergen".to_owned()),
                region: None,
                postal_code: Some("5009".to_owned()),
                country: Some("Norway".to_owned()),
                ..Address::default()
            },
        },
        RepositoryEventBody::UrlAdded {
            repository_id,
            url: Url {
                url_type: Some("home page".to_owned()),
                href: "https://www.arkivverket.no/".to_owned(),
                description: None,
            },
        },
        RepositoryEventBody::RestrictionsChanged {
            repository_id,
            restrictions: BTreeSet::from([Restriction::Locked]),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<RepositoryState>(repository_id, RepositoryEvent::new(&meta, body));
    }
}

/// The note created by the `note` module, which always runs before `repository`.
#[expect(clippy::expect_used, reason = "build_log always runs note before repository")]
fn existing_note_id(builder: &Builder) -> NoteId {
    builder.ids.note.expect("note exists before repository")
}

/// The tag created by the `tag` module, which always runs before `repository`.
#[expect(clippy::expect_used, reason = "build_log always runs tag before repository")]
fn existing_tag_id(builder: &Builder) -> TagId {
    builder.ids.tag.expect("tag exists before repository")
}

fn push_attachments(builder: &mut Builder, repository_id: RepositoryId, note_id: NoteId, tag_id: TagId) {
    let bodies = [
        RepositoryEventBody::NoteAttached { repository_id, note_id },
        RepositoryEventBody::Tagged { repository_id, tag_id },
        RepositoryEventBody::Untagged { repository_id, tag_id },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<RepositoryState>(repository_id, RepositoryEvent::new(&meta, body));
    }
}

fn push_corrections(builder: &mut Builder, repository_id: RepositoryId, target: AssertionId) {
    let bodies = [
        RepositoryEventBody::AssertionRetracted { repository_id, target },
        RepositoryEventBody::AssertionSuperseded { repository_id, target },
        RepositoryEventBody::HumanIdChanged {
            repository_id,
            human_id: HumanId::new("R0099"),
            old_human_id: HumanId::new("R0001"),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<RepositoryState>(repository_id, RepositoryEvent::new(&meta, body));
    }
}
