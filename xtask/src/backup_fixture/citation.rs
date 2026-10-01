//! Citation fixture events: every `CitationEventBody` variant.

use std::collections::BTreeSet;

use vitni_core::citation::{CitationEvent, CitationEventBody, CitationState};
use vitni_core::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use vitni_core::enums::Restriction;
use vitni_core::ids::{AssertionId, CitationId, HumanId, MediaId, NoteId, SourceId, TagId};
use vitni_core::provenance::{Confidence, EvidenceAnalysis, EvidenceKind, InformationKind, SourceQuality};
use vitni_core::text::{Attribute, MediaRef};

use crate::backup_fixture::Builder;

/// Pushes one citation through every variant, recording it in [`Builder::ids`].
pub(crate) fn events(builder: &mut Builder) {
    let citation_id = CitationId::from_uuid(builder.uuid());
    let created_id = push_created(builder, citation_id, existing_source_id(builder));
    push_analysis(builder, citation_id);
    let media_id = media_id(builder);
    push_attachments(
        builder,
        citation_id,
        media_id,
        existing_note_id(builder),
        existing_tag_id(builder),
    );
    push_corrections(builder, citation_id, created_id);
    builder.ids.citation = Some(citation_id);
}

fn push_created(builder: &mut Builder, citation_id: CitationId, source_id: SourceId) -> AssertionId {
    let meta = builder.meta();
    let assertion_id = meta.assertion_id;
    builder.push::<CitationState>(
        citation_id,
        CitationEvent::new(
            &meta,
            CitationEventBody::CitationCreated {
                citation_id,
                human_id: HumanId::new("C0001"),
                source_id,
            },
        ),
    );
    assertion_id
}

fn birth_record_date() -> GenealogicalDate {
    GenealogicalDate {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(1852),
            month: Some(4),
            day: Some(3),
        })),
        time: None,
        new_year_begins: None,
        sort_value: 18_520_403,
        original_text: Some("3. April 1852".to_owned()),
    }
}

fn push_analysis(builder: &mut Builder, citation_id: CitationId) {
    let bodies = [
        CitationEventBody::PageSet {
            citation_id,
            page: "fol. 12, entry 7".to_owned(),
        },
        CitationEventBody::DateAsserted {
            citation_id,
            date: birth_record_date(),
        },
        CitationEventBody::ConfidenceSet {
            citation_id,
            confidence: Confidence::High,
        },
        CitationEventBody::EvidenceAnalysisSet {
            citation_id,
            analysis: EvidenceAnalysis {
                source: SourceQuality::Original,
                information: InformationKind::Primary,
                evidence: EvidenceKind::Direct,
            },
        },
        CitationEventBody::AttributeAdded {
            citation_id,
            attribute: Attribute {
                attribute_type: "Legibility".to_owned(),
                value: "Good".to_owned(),
            },
        },
        CitationEventBody::RestrictionsChanged {
            citation_id,
            restrictions: BTreeSet::from([Restriction::Confidential]),
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<CitationState>(citation_id, CitationEvent::new(&meta, body));
    }
}

/// The media aggregate's id — minted here if `source` did not already mint it, since `citation` runs
/// before `media` in the fixed build order.
fn media_id(builder: &mut Builder) -> MediaId {
    if let Some(id) = builder.ids.media {
        return id;
    }
    let id = MediaId::from_uuid(builder.uuid());
    builder.ids.media = Some(id);
    id
}

/// The source created by the `source` module, which always runs before `citation`.
#[expect(clippy::expect_used, reason = "build_log always runs source before citation")]
fn existing_source_id(builder: &Builder) -> SourceId {
    builder.ids.source.expect("source exists before citation")
}

/// The note created by the `note` module, which always runs before `citation`.
#[expect(clippy::expect_used, reason = "build_log always runs note before citation")]
fn existing_note_id(builder: &Builder) -> NoteId {
    builder.ids.note.expect("note exists before citation")
}

/// The tag created by the `tag` module, which always runs before `citation`.
#[expect(clippy::expect_used, reason = "build_log always runs tag before citation")]
fn existing_tag_id(builder: &Builder) -> TagId {
    builder.ids.tag.expect("tag exists before citation")
}

fn push_attachments(builder: &mut Builder, citation_id: CitationId, media_id: MediaId, note_id: NoteId, tag_id: TagId) {
    let bodies = [
        CitationEventBody::MediaAttached {
            citation_id,
            media: MediaRef {
                media_id,
                crop: None,
                caption: Some("The relevant register page".to_owned()),
                citations: Vec::new(),
            },
        },
        CitationEventBody::NoteAttached { citation_id, note_id },
        CitationEventBody::Tagged { citation_id, tag_id },
        CitationEventBody::Untagged { citation_id, tag_id },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<CitationState>(citation_id, CitationEvent::new(&meta, body));
    }
}

/// The citation's corrections and identity decisions: merged with one duplicate copy and distinguished
/// from another, both minted here — the projection needs neither to exist.
fn push_corrections(builder: &mut Builder, citation_id: CitationId, target: AssertionId) {
    let (duplicate, other) = (
        CitationId::from_uuid(builder.uuid()),
        CitationId::from_uuid(builder.uuid()),
    );
    let bodies = [
        CitationEventBody::AssertionRetracted { citation_id, target },
        CitationEventBody::AssertionSuperseded { citation_id, target },
        CitationEventBody::HumanIdChanged {
            citation_id,
            human_id: HumanId::new("C0099"),
            old_human_id: HumanId::new("C0001"),
        },
        CitationEventBody::CitationsMerged {
            surviving: citation_id,
            merged: duplicate,
            assessment: None,
        },
        CitationEventBody::CitationsDistinguished {
            citation: citation_id,
            other,
            assessment: None,
        },
    ];
    for body in bodies {
        let meta = builder.meta();
        builder.push::<CitationState>(citation_id, CitationEvent::new(&meta, body));
    }
}
