//! SSR assertions for the Media detail (Phase 5 PR10): render the overview (preview + File card +
//! the "Used by" reverse-index card), the citations table (source · page · surety · evidence axes),
//! and the tags panel. Asserts the file metadata, the back-reference rows, the confidence/evidence
//! cues, and that a tag shows its name/colour but never its id.

use dioxus::prelude::*;
use genealogy_app::{TagRef, UsingKind};
use genealogy_ui::{
    CitationRefVm, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm, Localizer, MediaAttributeVm, MediaDetail,
    UsingRecordVm,
};
use genealogy_ui_dioxus::screens::{MediaEditForm, media_citations_table, media_overview, media_tags_panel};

/// A representative media detail: a portrait JPEG with file metadata, one backing citation (Normal
/// surety, an Original evidence axis), one referencing record (a person, as a portrait), and one tag.
fn sample() -> MediaDetail {
    MediaDetail {
        human_id: "O0050".to_owned(),
        id: "0190-media-id".to_owned(),
        title: "john-smith-portrait.jpg".to_owned(),
        path: Some("media/portraits/john-smith-portrait.jpg".to_owned()),
        mime: Some("image/jpeg".to_owned()),
        checksum: Some("sha256:9f3a8c12d4e7b6a05f1e".to_owned()),
        date: Some("c. 1900".to_owned()),
        attributes: vec![MediaAttributeVm {
            attribute_type: "dimensions".to_owned(),
            value: "1024x1536".to_owned(),
        }],
        citations: vec![CitationRefVm {
            human_id: "C0007".to_owned(),
            source: Some("Family photo collection".to_owned()),
            source_id: None,
            page: Some("album 2, p.4".to_owned()),
            confidence: Some(ConfidenceLevel::Normal),
            confidence_label: Some("Normal".to_owned()),
            evidence_axes: vec![EvidenceAxisVm {
                axis: EvidenceAxis::Source,
                label: "Original".to_owned(),
            }],
            asserted_by: None,
        }],
        notes: Vec::new(),
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Direct ancestor".to_owned(),
            color: Some("#e5534b".to_owned()),
            priority: Some(1),
        }],
        used_by: vec![UsingRecordVm {
            kind: UsingKind::Person,
            human_id: "I0042".to_owned(),
            id: "0190-person-42".to_owned(),
            label: "John Smith".to_owned(),
            kind_label: "Person".to_owned(),
        }],
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

/// Renders the overview, citations, and tags tabs together.
fn media_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let editing = use_signal(|| None::<MediaEditForm>);
    let on_submit = use_callback(|_edit: genealogy_ui::MediaEdit| {});
    let detail = sample();
    rsx! {
        {media_overview(&loc, &detail, editing)}
        {media_citations_table(&loc, &detail.citations)}
        {media_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

#[test]
fn overview_shows_file_metadata_and_used_by_back_references() {
    let mut vdom = VirtualDom::new(media_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "media/portraits/john-smith-portrait.jpg", // file path
        "image/jpeg",                              // mime
        "sha256:9f3a8c12d4e7b6a05f1e",             // checksum
        "John Smith",                              // the using-record label
        "Person",                                  // the using-record kind chip
        "I0042",                                   // the using-record human id
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn citations_carry_source_page_and_evidence() {
    let mut vdom = VirtualDom::new(media_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "Family photo collection", // the cited source
        "album 2, p.4",            // the citation page
        r#"data-level="normal""#,  // the surety badge colour token
        ">Normal",                 // the surety label (colour is never the only signal)
        "Original",                // an evidence axis on the citation
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let mut vdom = VirtualDom::new(media_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("Direct ancestor"), "tag name shown:\n{html}");
    assert!(html.contains("#e5534b"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
