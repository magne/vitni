//! SSR assertions for the Citation detail (Phase 5 PR27): the read-first Overview record (id · source
//! · date · page · confidence · evidence axes), its edit mode swapping in inputs plus the sticky-header
//! Cancel/Save, the attributes table, and the tags panel (name/colour, never id).

use dioxus::prelude::*;
use vitni_app::{
    Calendar, DateInput, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody, Rect, TagRef,
    build_genealogical_date,
};
use vitni_ui::{
    AttachedRefVm, CitationAttributeVm, CitationDetail, CitationDraft, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm,
    EvidenceKind, InformationKind, Localizer, MediaRefVm, ProvenanceDraft, SourceQuality,
};

/// The sample citation's structured cited-record date: exact 1880 on the Gregorian calendar.
fn sample_date() -> GenealogicalDate {
    build_genealogical_date(DateInput {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        body: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(1880),
            month: None,
            day: None,
        })),
        new_year_begins: None,
        original_text: None,
        time: None,
    })
}
use vitni_ui_dioxus::screens::{
    CitationEditForm, MediaTabState, RecordActionLabels, RecordEditState, citation_attributes_table, citation_overview,
    id_list, media_gallery, media_tab, record_head_actions, tags_panel,
};

/// A representative citation detail: a cited source, High confidence, all three evidence axes, an
/// attribute, and one applied tag (name + colour + a hidden id).
fn sample() -> CitationDetail {
    CitationDetail {
        human_id: "C0001".to_owned(),
        source: Some("S0001".to_owned()),
        page: Some("p. 42".to_owned()),
        date: Some("1880".to_owned()),
        date_value: Some(sample_date()),
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
        attributes: vec![CitationAttributeVm {
            attribute_type: "quality".to_owned(),
            value: "good".to_owned(),
            assertion_id: "0190-attr-assert-1".to_owned(),
        }],
        media: vec![MediaRefVm {
            human_id: "O0004".to_owned(),
            caption: None,
            crop: None,
            path: None,
            mime: None,
            assertion_id: "0190-media-attach-1".to_owned(),
        }],
        notes: vec![AttachedRefVm {
            human_id: "N0004".to_owned(),
            assertion_id: "0190-note-attach-1".to_owned(),
        }],
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
    let on_remove = use_callback(|_: String| {});
    let onedit = use_callback(|_: CitationEditForm| {});
    let onretract = use_callback(|_: (String, String, bool)| {});
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (CitationDraft, ProvenanceDraft)| {}))}
        {citation_overview(&loc, &detail, record)}
        {citation_attributes_table(&loc, &detail.attributes, onedit, onretract)}
        {media_gallery(&loc, &detail.media, Some(onretract), None)}
        {id_list(&loc, &detail.notes, Some(onretract))}
        {tags_panel(&loc, &detail.tags, editing, CitationEditForm::Tag, on_remove)}
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

#[test]
fn attribute_rows_carry_edit_and_retract_with_row_scoped_labels() {
    let html = render(citation_view);
    assert!(
        html.contains(r#"aria-label="Edit quality""#),
        "the attribute row Edit carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract quality""#),
        "the attribute row Retract carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains("Retract this assertion — it stays in History"),
        "the Retract button carries the retract-title tooltip:\n{html}"
    );
}

#[test]
fn notes_and_media_carry_detach() {
    let html = render(citation_view);
    assert!(
        html.contains(r#"aria-label="Detach O0004""#),
        "the attached media carries a Detach:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Detach N0004""#),
        "the attached note carries a Detach:\n{html}"
    );
}

#[test]
fn no_assertion_id_is_ever_rendered() {
    let html = render(citation_view);
    for assertion_id in [
        "0190-attr-assert-1",
        "0190-media-attach-1",
        "0190-note-attach-1",
        "0190-secret-tag-id",
    ] {
        assert!(
            !html.contains(assertion_id),
            "assertion/tag id {assertion_id:?} must never be rendered:\n{html}"
        );
    }
}

/// The Citation Media tab (issue #199): the gallery card opens the shared crop viewer, mirroring the
/// Person screen's `media_tab` wiring (`citation.rs`'s "media" tab arm).
fn citation_media_tab_view() -> Element {
    let loc = loc();
    let on_retract = use_callback(|_target: (String, String, bool)| {});
    let detail = sample();
    let viewing = use_signal(|| detail.media.first().cloned());
    let on_view = use_callback(|_item: MediaRefVm| {});
    let on_region = use_callback(|_region: (String, Option<Rect>, Option<String>)| {});
    let media_state = MediaTabState {
        viewing,
        on_view,
        on_region,
    };
    rsx! {
        {media_tab(&loc, &detail.media, Some(on_retract), media_state)}
    }
}

#[test]
fn media_tab_opens_the_crop_viewer_on_a_card_click() {
    let html = render(citation_media_tab_view);
    assert!(
        html.contains("media-open"),
        "the gallery card opens the crop viewer (ADR 0017 §GUI):\n{html}"
    );
    assert!(
        html.contains("Set region") && html.contains("Clear region"),
        "the crop viewer overlay renders with its Set/Clear region actions:\n{html}"
    );
}
