//! SSR assertions for the Citation detail (Phase 5 PR27): the read-first Overview record (id · source
//! · date · page · confidence · evidence axes), its edit mode swapping in inputs plus the sticky-header
//! Cancel/Save, the attributes table, and the tags panel (name/colour, never id).

use dioxus::prelude::*;
use genealogy_app::TagRef;
use genealogy_ui::{
    CitationDetail, CitationDraft, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm, EvidenceKind, InformationKind,
    Localizer, ProvenanceDraft, SourceQuality,
};
use genealogy_ui_dioxus::screens::{
    CitationEditForm, RecordActionLabels, RecordEditState, citation_attributes_table, citation_overview,
    citation_tags_panel, record_head_actions,
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
        source_quality: Some(SourceQuality::Original),
        information: Some(InformationKind::Primary),
        evidence_kind: Some(EvidenceKind::Direct),
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

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// A record edit state seeded from the sample, in view or edit mode.
fn state(editing: bool) -> RecordEditState<CitationDraft> {
    let seed = CitationDraft::from_detail(&sample());
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

fn citation_view() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(false);
    let editing = use_signal(|| None::<CitationEditForm>);
    let on_submit = use_callback(|_edit: (genealogy_ui::CitationEdit, genealogy_ui::ProvenanceDraft)| {});
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (CitationDraft, ProvenanceDraft)| {}))}
        {citation_overview(&loc, &detail, record)}
        {citation_attributes_table(&loc, &detail.attributes)}
        {citation_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

fn citation_edit() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(true);
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (CitationDraft, ProvenanceDraft)| {}))}
        {citation_overview(&loc, &detail, record)}
    }
}

#[test]
fn overview_is_read_first_with_an_edit_button_and_no_inputs() {
    let html = render(citation_view);
    assert!(html.contains(">Edit<"), "view mode offers Edit in the header:\n{html}");
    assert!(
        !html.contains("<input"),
        "view mode shows read boxes, not inputs:\n{html}"
    );
    assert!(html.contains("S0001"), "the cited source is shown:\n{html}");
    assert!(
        html.contains("Original") && html.contains("Direct"),
        "the evidence axes chips show:\n{html}"
    );
    assert!(
        html.contains("quality") && html.contains("good"),
        "the attribute row shows:\n{html}"
    );
}

#[test]
fn edit_mode_swaps_in_the_inputs_and_header_actions() {
    let html = render(citation_edit);
    assert!(html.contains("<input"), "edit mode swaps in inputs:\n{html}");
    assert!(
        html.contains("<select"),
        "edit mode swaps in the confidence/axes selects:\n{html}"
    );
    assert!(
        html.contains(">Cancel<") && html.contains(">Save<"),
        "Cancel/Save in the header:\n{html}"
    );
    assert!(
        html.contains(r#"id="citation-id""#),
        "the editable human id is present:\n{html}"
    );
    assert!(html.contains(r#"value="p. 42""#), "the page input is seeded:\n{html}");
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let html = render(citation_view);
    assert!(html.contains("Direct ancestor"), "tag name shown:\n{html}");
    assert!(html.contains("#e5534b"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
