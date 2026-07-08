//! SSR assertions for the DNA-match detail (Phase 5 PR27): the read-first Overview record (id · the
//! locked observed totals · the editable confirmation status), its edit mode swapping in the status
//! select while the observations render as disabled inputs (§3), the segments + shared-ancestors
//! tables, and the tags panel (name/colour, never id).

use dioxus::prelude::*;
use genealogy_app::{MatchStatus, TagRef, UsingKind};
use genealogy_ui::{
    AttachedRefVm, DnaMatchDetail, DnaMatchDraft, DnaSegmentVm, Localizer, ProvenanceDraft, SharedAncestorVm,
    UsingRecordVm,
};
use genealogy_ui_dioxus::screens::{
    DnaMatchEditForm, RecordActionLabels, RecordEditState, dna_match_ancestors_table, dna_match_overview,
    dna_match_segments_table, dna_match_tags_panel, record_head_actions,
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
        status: "Confirmed".to_owned(),
        status_kind: Some(MatchStatus::Confirmed),
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
        notes: vec![AttachedRefVm {
            human_id: "N0004".to_owned(),
            assertion_id: "01920000-0000-7000-8000-0000000000d4".to_owned(),
        }],
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

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn state(editing: bool) -> RecordEditState<DnaMatchDraft> {
    let seed = DnaMatchDraft::from_detail(&sample());
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

fn dna_match_view() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(false);
    let editing = use_signal(|| None::<DnaMatchEditForm>);
    let on_submit = use_callback(|_edit: (genealogy_ui::DnaMatchEdit, genealogy_ui::ProvenanceDraft)| {});
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (DnaMatchDraft, ProvenanceDraft)| {}))}
        {dna_match_overview(&loc, &detail, record)}
        {dna_match_segments_table(&loc, &detail.segments)}
        {dna_match_ancestors_table(&loc, &detail.shared_ancestors)}
        {dna_match_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

fn dna_match_edit() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(true);
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (DnaMatchDraft, ProvenanceDraft)| {}))}
        {dna_match_overview(&loc, &detail, record)}
    }
}

#[test]
fn overview_is_read_first_with_an_edit_button_and_no_inputs() {
    let html = render(dna_match_view);
    assert!(html.contains(">Edit<"), "view mode offers Edit in the header:\n{html}");
    assert!(
        !html.contains("<input"),
        "view mode shows read boxes, not inputs:\n{html}"
    );
    for needle in [
        "John Smith",
        "Mary Doe",
        "1750",
        "24.9",
        "Aunt / Niece / Half-sibling",
        "Confirmed",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn locked_fields_render_disabled_inputs() {
    let html = render(dna_match_edit);
    assert!(html.contains("<input"), "edit mode swaps in inputs:\n{html}");
    assert!(
        html.contains("<select"),
        "the confirmation status is an editable select:\n{html}"
    );
    assert!(
        html.contains(r#"id="dna-match-id""#),
        "the editable human id is present:\n{html}"
    );
    assert!(
        html.contains(r#"id="dna-match-shared-cm""#) && html.contains("disabled"),
        "the observed totals render as locked (disabled) inputs:\n{html}"
    );
    assert!(
        html.contains(r#"id="dna-match-test-a""#),
        "the compared tests render locked:\n{html}"
    );
    assert!(
        html.contains(">Cancel<") && html.contains(">Save<"),
        "Cancel/Save in the header:\n{html}"
    );
}

#[test]
fn segments_and_shared_ancestors_are_listed() {
    let html = render(dna_match_view);
    for needle in ["742429", "28104553", "paternal", "Thomas Smith", "Paternal grandfather"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let html = render(dna_match_view);
    assert!(html.contains("Needs phasing"), "tag name shown:\n{html}");
    assert!(html.contains("#e0884a"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
