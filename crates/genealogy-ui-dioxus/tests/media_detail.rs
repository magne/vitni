//! SSR assertions for the Media detail (Phase 5 PR27): the read-first Overview record (id · paths ·
//! MIME, with checksum/date locked), its edit mode swapping in inputs (checksum/date disabled) plus
//! the sticky-header Cancel/Save, the citations table, and the tags panel.

use dioxus::prelude::*;
use genealogy_app::{TagRef, UsingKind};
use genealogy_ui::{
    CitationRefVm, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm, Localizer, MediaAttributeVm, MediaDetail, MediaDraft,
    ProvenanceDraft, UsingRecordVm,
};
use genealogy_ui_dioxus::screens::{
    RecordActionLabels, RecordEditState, media_citations_table, media_overview, media_tags_panel, record_head_actions,
};

/// A representative media detail: a portrait JPEG with file metadata, one backing citation (Normal
/// surety, an Original evidence axis), one referencing record (a person), and one tag.
fn sample() -> MediaDetail {
    MediaDetail {
        human_id: "O0050".to_owned(),
        id: "0190-media-id".to_owned(),
        title: "john-smith-portrait.jpg".to_owned(),
        path: Some("media/portraits/john-smith-portrait.jpg".to_owned()),
        file_path: Some("media/portraits/john-smith-portrait.jpg".to_owned()),
        web_path: None,
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

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn state(editing: bool) -> RecordEditState<MediaDraft> {
    let seed = MediaDraft::from_detail(&sample());
    RecordEditState {
        editing: use_signal(move || editing),
        seed: use_signal({
            let seed = seed.clone();
            move || seed
        }),
        draft: use_signal(move || seed),
        prov: use_signal(ProvenanceDraft::default),
    }
}

fn media_view() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(false);
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (MediaDraft, ProvenanceDraft)| {}))}
        {media_overview(&loc, &detail, record)}
        {media_citations_table(&loc, &detail.citations)}
        {media_tags_panel(&loc, &detail, use_signal(|| None), use_callback(|_| {}), &detail.human_id)}
    }
}

fn media_edit() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(true);
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (MediaDraft, ProvenanceDraft)| {}))}
        {media_overview(&loc, &detail, record)}
    }
}

#[test]
fn overview_is_read_first_with_an_edit_button_and_no_inputs() {
    let html = render(media_view);
    assert!(html.contains(">Edit<"), "view mode offers Edit:\n{html}");
    assert!(!html.contains("<input"), "no live inputs in view mode:\n{html}");
    for needle in [
        "media/portraits/john-smith-portrait.jpg",
        "image/jpeg",
        "sha256:9f3a8c12d4e7b6a05f1e",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn edit_mode_swaps_in_the_inputs_and_header_actions() {
    let html = render(media_edit);
    assert!(html.contains("<input"), "edit mode swaps in inputs:\n{html}");
    assert!(
        html.contains(">Cancel<") && html.contains(">Save<"),
        "Cancel/Save in the header:\n{html}"
    );
    assert!(
        html.contains(r#"id="media-file-path""#),
        "the file-path input is present:\n{html}"
    );
    assert!(
        html.contains(r#"id="media-mime""#),
        "the MIME input is present:\n{html}"
    );
}

#[test]
fn locked_fields_render_disabled_inputs() {
    let html = render(media_edit);
    assert!(
        html.contains(r#"id="media-checksum""#),
        "the checksum field is present:\n{html}"
    );
    assert!(
        html.contains(r#"id="media-date""#),
        "the date field is present:\n{html}"
    );
    assert!(
        html.contains("disabled"),
        "the locked checksum/date render disabled inputs:\n{html}"
    );
    assert!(
        html.contains(r#"value="sha256:9f3a8c12d4e7b6a05f1e""#),
        "the locked checksum is seeded from the record:\n{html}"
    );
}

#[test]
fn citations_carry_source_page_and_evidence() {
    let html = render(media_view);
    for needle in [
        "Family photo collection",
        "album 2, p.4",
        r#"data-level="normal""#,
        ">Normal",
        "Original",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let html = render(media_view);
    assert!(html.contains("Direct ancestor"), "tag name shown:\n{html}");
    assert!(html.contains("#e5534b"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
