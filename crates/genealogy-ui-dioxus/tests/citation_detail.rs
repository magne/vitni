//! SSR assertions for the Citation detail (Phase 5 PR6): render the overview (the research-grade
//! Evidence Explained axes + confidence badge + source), the attributes table, and the tags panel,
//! and assert the evidence cues, table roles, and that a tag shows its name/colour but never its id.
//! Pure render-and-inspect — the same pattern as `person_detail.rs` / `components.rs`.

use dioxus::prelude::*;
use genealogy_app::TagRef;
use genealogy_ui::{CitationDetail, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm, Localizer};
use genealogy_ui_dioxus::screens::{
    CitationEditForm, citation_attributes_table, citation_overview, citation_tags_panel,
};

/// A representative citation detail: a cited source, High confidence, all three evidence axes, an
/// attribute, and one applied tag (name + colour + a hidden id).
fn sample() -> CitationDetail {
    CitationDetail {
        human_id: "C0001".to_owned(),
        source: Some("S0001".to_owned()),
        page: Some("p. 42".to_owned()),
        date: Some("1880".to_owned()),
        confidence: Some(ConfidenceLevel::High),
        confidence_label: Some("High".to_owned()),
        evidence_axes: vec![
            EvidenceAxisVm {
                axis: EvidenceAxis::Source,
                label: "Original".to_owned(),
            },
            EvidenceAxisVm {
                axis: EvidenceAxis::Information,
                label: "Primary".to_owned(),
            },
            EvidenceAxisVm {
                axis: EvidenceAxis::Evidence,
                label: "Direct".to_owned(),
            },
        ],
        restrictions: Vec::new(),
        attributes: vec![("quality".to_owned(), "good".to_owned())],
        media: Vec::new(),
        notes: Vec::new(),
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Direct ancestor".to_owned(),
            color: Some("#e5534b".to_owned()),
            priority: Some(1),
        }],
        history: Vec::new(),
    }
}

/// Renders the overview, the attributes table, and the tags panel together (the tag panel needs a
/// reactive scope for its editing signal + submit callback, so this runs inside a `VirtualDom`).
fn citation_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let editing = use_signal(|| None::<CitationEditForm>);
    let on_submit = use_callback(|_edit: genealogy_ui::CitationEdit| {});
    let detail = sample();
    rsx! {
        {citation_overview(&loc, &detail, editing)}
        {citation_attributes_table(&loc, &detail.attributes)}
        {citation_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

#[test]
fn overview_renders_evidence_axes_confidence_and_source() {
    let mut vdom = VirtualDom::new(citation_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "S0001",                // the cited source
        r#"data-level="high""#, // confidence colour token
        ">High",                // confidence label — colour is never the only signal
        "ev source",            // evidence-axis chip class (the source axis)
        "Original",             // the source-axis value
        "Primary",              // the information-axis value
        "Direct",               // the evidence-axis value
        r#"class="tbl""#,       // the attributes table
        "quality",              // the attribute type
        "good",                 // the attribute value
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let mut vdom = VirtualDom::new(citation_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("Direct ancestor"), "tag name shown:\n{html}");
    assert!(html.contains("#e5534b"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
