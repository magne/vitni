//! Tag fixture events: every `TagEventBody` variant.

use std::collections::BTreeSet;

use vitni_core::enums::Restriction;
use vitni_core::ids::TagId;
use vitni_core::tag::{TagEvent, TagEventBody, TagState};

use crate::backup_fixture::Builder;

/// Pushes one tag through every variant, recording it in [`Builder::ids`].
pub(crate) fn events(builder: &mut Builder) {
    let tag_id = TagId::from_uuid(builder.uuid());
    let bodies = [
        TagEventBody::TagCreated {
            tag_id,
            name: "Emigrants".to_owned(),
        },
        TagEventBody::TagRenamed {
            tag_id,
            name: "Emigrated to America".to_owned(),
        },
        TagEventBody::TagColorSet {
            tag_id,
            color: "#1f77b4".to_owned(),
        },
        TagEventBody::TagPrioritySet { tag_id, priority: 3 },
        TagEventBody::RestrictionsChanged {
            tag_id,
            restrictions: BTreeSet::from([Restriction::Confidential]),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<TagState>(tag_id, TagEvent::new(&meta, body));
    }
    builder.ids.tag = Some(tag_id);
}
