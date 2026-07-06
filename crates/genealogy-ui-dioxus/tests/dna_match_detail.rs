//! SSR assertions for the DNA-match detail (Phase 5 PR11): render the overview (compared tests +
//! observed shared DNA + inferred relationship), the segments table, the shared-ancestors table, and
//! the tags panel. Asserts the observation/conclusion split, the segments, the ancestors, and that a
//! tag shows its name/colour but never its id.

use dioxus::prelude::*;
use genealogy_app::{TagRef, UsingKind};
use genealogy_ui::{DnaMatchDetail, DnaSegmentVm, Localizer, SharedAncestorVm, UsingRecordVm};
use genealogy_ui_dioxus::screens::{
    DnaMatchEditForm, dna_match_ancestors_table, dna_match_overview, dna_match_segments_table, dna_match_tags_panel,
};

fn test_ref(human_id: &str, label: &str) -> UsingRecordVm {
    UsingRecordVm {
        kind: UsingKind::DnaTest,
        human_id: human_id.to_owned(),
        id: format!("0190-{human_id}"),
        label: label.to_owned(),
        kind_label: "DNA test".to_owned(),
    }
}

/// A representative DNA match: John Smith ⟷ Mary Doe, 1,750 cM, one segment, one shared ancestor, one tag.
fn sample() -> DnaMatchDetail {
    DnaMatchDetail {
        human_id: "X0001".to_owned(),
        id: "0190-match-1".to_owned(),
        title: "John Smith ⟷ Mary Doe".to_owned(),
        test_a: Some(test_ref("D0002", "John Smith")),
        test_b: Some(test_ref("D0005", "Mary Doe")),
        provider: Some("AncestryDNA".to_owned()),
        shared_cm: Some("1750".to_owned()),
        percent_shared: Some("24.9".to_owned()),
        largest_segment_cm: Some("120".to_owned()),
        predicted_relationship: Some("Aunt / Niece / Half-sibling".to_owned()),
        status: "Normal".to_owned(),
        segments: vec![DnaSegmentVm {
            chromosome: "1".to_owned(),
            start: "742429".to_owned(),
            end: "28104553".to_owned(),
            centimorgans: "120".to_owned(),
            snps: Some("9842".to_owned()),
            side: "paternal".to_owned(),
        }],
        shared_ancestors: vec![SharedAncestorVm {
            person: Some(UsingRecordVm {
                kind: UsingKind::Person,
                human_id: "I0099".to_owned(),
                id: "0190-person-99".to_owned(),
                label: "Thomas Smith".to_owned(),
                kind_label: "Person".to_owned(),
            }),
            note: Some("Paternal grandfather".to_owned()),
        }],
        notes: vec!["N0004".to_owned()],
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Needs phasing".to_owned(),
            color: Some("#e0884a".to_owned()),
            priority: Some(2),
        }],
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

/// Renders the overview, segments, ancestors, and tags tabs together.
fn dna_match_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let editing = use_signal(|| None::<DnaMatchEditForm>);
    let on_submit = use_callback(|_edit: (genealogy_ui::DnaMatchEdit, genealogy_ui::ProvenanceDraft)| {});
    let detail = sample();
    rsx! {
        {dna_match_overview(&loc, &detail, on_submit, &detail.human_id)}
        {dna_match_segments_table(&loc, &detail.segments)}
        {dna_match_ancestors_table(&loc, &detail.shared_ancestors)}
        {dna_match_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

#[test]
fn overview_shows_compared_tests_shared_dna_and_inferred_relationship() {
    let mut vdom = VirtualDom::new(dna_match_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "John Smith",
        "Mary Doe",
        "1750",
        "24.9",
        "Aunt / Niece / Half-sibling",
        "Normal",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn segments_and_shared_ancestors_are_listed() {
    let mut vdom = VirtualDom::new(dna_match_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in ["742429", "28104553", "paternal", "Thomas Smith", "Paternal grandfather"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let mut vdom = VirtualDom::new(dna_match_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("Needs phasing"), "tag name shown:\n{html}");
    assert!(html.contains("#e0884a"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
