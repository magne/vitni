//! SSR assertions for the Family detail (Phase 5 PR27): the read-first Overview (Partners + Marriage
//! cards with the evidence-first cues), its edit mode swapping in the family's only scalar — the
//! editable id — plus the sticky-header Cancel/Save, the children + events tables, and the tags panel
//! (name/colour, never id).

use dioxus::prelude::*;
use genealogy_app::{ChildParentRelationship, TagRef};
use genealogy_ui::{
    CitationRefVm, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm, FamilyChildVm, FamilyDetail, FamilyDraft,
    FamilyEventVm, FamilyMediaVm, Localizer, PartnerVm, ProvenanceDraft,
};
use genealogy_ui_dioxus::screens::{
    FamilyEditForm, RecordActionLabels, RecordEditState, family_children_table, family_events_table, family_overview,
    family_tags_panel, id_list, record_head_actions,
};

/// A representative marriage-register citation, used to back the partner + marriage provenance cues.
fn marriage_citation() -> CitationRefVm {
    CitationRefVm {
        human_id: "C0001".to_owned(),
        source: Some("Trinity Church marriage register".to_owned()),
        source_id: Some("S0003".to_owned()),
        page: Some("vol. 5, f. 18".to_owned()),
        confidence: Some(ConfidenceLevel::High),
        confidence_label: Some("High".to_owned()),
        evidence_axes: vec![EvidenceAxisVm {
            axis: EvidenceAxis::Source,
            label: "Original".to_owned(),
        }],
        asserted_by: Some("asserted by magne · 2026-06-21 16:05".to_owned()),
        assertion_id: None,
    }
}

fn sample() -> FamilyDetail {
    FamilyDetail {
        human_id: "F0017".to_owned(),
        id: "0190-family-id".to_owned(),
        title: "Mary Doe & John Smith".to_owned(),
        partners: vec![
            PartnerVm {
                human_id: "I0001".to_owned(),
                name: "Mary Doe".to_owned(),
                vitals: Some("1852 – 1921".to_owned()),
                source_count: 1,
                citations: vec![marriage_citation()],
                assertion_id: "01920000-0000-7000-8000-0000000000a1".to_owned(),
            },
            PartnerVm {
                human_id: "I0002".to_owned(),
                name: "John Smith".to_owned(),
                vitals: None,
                source_count: 0,
                citations: Vec::new(),
                assertion_id: "01920000-0000-7000-8000-0000000000a2".to_owned(),
            },
        ],
        marriage: Some(FamilyEventVm {
            human_id: "E0001".to_owned(),
            type_label: "Marriage".to_owned(),
            date: Some("14 Jun 1876".to_owned()),
            place: Some("Trinity Church, New York".to_owned()),
            confidence: ConfidenceLevel::High,
            confidence_label: "High".to_owned(),
            source_count: 1,
            citations: vec![marriage_citation()],
            assertion_id: "01920000-0000-7000-8000-0000000000e5".to_owned(),
        }),
        children: vec![FamilyChildVm {
            human_id: "I0003".to_owned(),
            name: "Jonathan Smith".to_owned(),
            born: Some("1878".to_owned()),
            relationships: vec![
                ("I0001".to_owned(), "Birth".to_owned()),
                ("I0002".to_owned(), "Step".to_owned()),
            ],
            relationship_kinds: vec![
                ("I0001".to_owned(), ChildParentRelationship::Birth),
                ("I0002".to_owned(), ChildParentRelationship::Step),
            ],
            confidence: ConfidenceLevel::Normal,
            confidence_label: "Normal".to_owned(),
            source_count: 0,
            assertion_id: "01920000-0000-7000-8000-0000000000c3".to_owned(),
        }],
        events: vec![FamilyEventVm {
            human_id: "E0001".to_owned(),
            type_label: "Marriage".to_owned(),
            date: Some("14 Jun 1876".to_owned()),
            place: Some("Trinity Church, New York".to_owned()),
            confidence: ConfidenceLevel::High,
            confidence_label: "High".to_owned(),
            source_count: 1,
            citations: Vec::new(),
            assertion_id: "01920000-0000-7000-8000-0000000000e5".to_owned(),
        }],
        media: vec![FamilyMediaVm {
            human_id: "O0001".to_owned(),
            caption: Some("Wedding portrait, 1876".to_owned()),
            assertion_id: "01920000-0000-7000-8000-0000000000f1".to_owned(),
        }],
        notes: Vec::new(),
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Ancestral line".to_owned(),
            color: Some("#74b449".to_owned()),
            priority: Some(1),
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

fn state(editing: bool) -> RecordEditState<FamilyDraft> {
    let seed = FamilyDraft::from_detail(&sample());
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

fn family_view() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(false);
    let editing = use_signal(|| None::<FamilyEditForm>);
    let on_submit = use_callback(|_edit: (genealogy_ui::FamilyEdit, genealogy_ui::ProvenanceDraft)| {});
    let on_retract = use_callback(|_target: (String, String, bool)| {});
    let on_edit_open = use_callback(|_form: FamilyEditForm| {});
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (FamilyDraft, ProvenanceDraft)| {}))}
        {family_overview(&loc, &detail, editing, record, on_retract)}
        {family_children_table(&loc, &detail, on_edit_open, on_retract)}
        {family_events_table(&loc, &detail.events, on_retract)}
        {family_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

/// A notes list with an armed detach callback — exercises the attachment Detach affordance.
fn family_notes_detach() -> Element {
    let loc = loc();
    let on_retract = use_callback(|_target: (String, String, bool)| {});
    let notes = vec![genealogy_ui::AttachedRefVm {
        human_id: "N0007".to_owned(),
        assertion_id: "01920000-0000-7000-8000-0000000000a7".to_owned(),
    }];
    rsx! {
        {id_list(&loc, &notes, Some(on_retract))}
    }
}

fn family_edit() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(true);
    let editing = use_signal(|| None::<FamilyEditForm>);
    let on_retract = use_callback(|_target: (String, String, bool)| {});
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (FamilyDraft, ProvenanceDraft)| {}))}
        {family_overview(&loc, &detail, editing, record, on_retract)}
    }
}

#[test]
fn overview_is_read_first_with_an_edit_button_and_no_inputs() {
    let html = render(family_view);
    assert!(html.contains(">Edit<"), "view mode offers Edit in the header:\n{html}");
    assert!(
        !html.contains("<input"),
        "view mode shows read boxes, not inputs:\n{html}"
    );
    for needle in ["Mary Doe", "1852 – 1921", "John Smith", "no-source", "14 Jun 1876"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn edit_mode_swaps_in_the_editable_id_and_header_actions() {
    let html = render(family_edit);
    assert!(html.contains("<input"), "edit mode swaps in the id input:\n{html}");
    assert!(
        html.contains(r#"id="family-id""#),
        "the editable human id is present:\n{html}"
    );
    assert!(html.contains(r#"value="F0017""#), "the id input is seeded:\n{html}");
    assert!(
        html.contains(">Cancel<") && html.contains(">Save<"),
        "Cancel/Save in the header:\n{html}"
    );
}

#[test]
fn children_table_has_a_relationship_column_per_partner() {
    let html = render(family_view);
    for needle in [
        r#"class="tbl""#,
        "Jonathan Smith",
        "1878",
        "Birth",
        "Step",
        r#"data-level="normal""#,
        "Marriage",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let html = render(family_view);
    assert!(html.contains("Ancestral line"), "tag name shown:\n{html}");
    assert!(html.contains("#74b449"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}

#[test]
fn partner_rows_offer_remove_with_row_scoped_labels_and_no_id_leak() {
    let html = render(family_view);
    assert!(html.contains(">Remove<"), "partners offer a Remove verb:\n{html}");
    assert!(
        html.contains(r#"title="Remove this partner"#),
        "the Remove tooltip is the mockup sentence:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Remove Mary Doe""#),
        "Remove carries a row-scoped accessible name:\n{html}"
    );
    for secret in [
        "01920000-0000-7000-8000-0000000000a1",
        "01920000-0000-7000-8000-0000000000a2",
    ] {
        assert!(
            !html.contains(secret),
            "a partner assertion id leaked: {secret}\n{html}"
        );
    }
}

#[test]
fn children_rows_offer_edit_and_remove_with_row_scoped_labels() {
    let html = render(family_view);
    // Edit opens the child form pre-filled; Remove retracts the child assertion (it stays in History).
    assert!(html.contains(">Edit<"), "the visible Edit verb:\n{html}");
    assert!(html.contains(">Remove<"), "the visible Remove verb:\n{html}");
    assert!(
        html.contains(r#"aria-label="Edit Jonathan Smith""#),
        "Edit carries a row-scoped accessible name:\n{html}"
    );
    // The Remove button carries the mockup tooltip; the apostrophe-free sentence is matched as a prefix.
    assert!(
        html.contains(r#"title="Remove this child"#),
        "the Remove tooltip is the mockup sentence:\n{html}"
    );
    // The row-scoped accessible name uses the Remove verb (matching the visible label).
    assert!(
        html.contains(r#"aria-label="Remove Jonathan Smith""#),
        "Remove carries a verb-correct row-scoped accessible name:\n{html}"
    );
}

#[test]
fn events_rows_offer_unlink_only_and_no_edit() {
    let html = render(family_view);
    assert!(html.contains(">Unlink<"), "events offer Unlink:\n{html}");
    assert!(
        html.contains(r#"title="Unlink this family event"#),
        "the Unlink tooltip is the mockup sentence:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Unlink Marriage""#),
        "Unlink carries a verb-correct row-scoped accessible name:\n{html}"
    );
    assert!(
        !html.contains(r#"aria-label="Edit Marriage""#),
        "a family event has no per-row Edit — the link is unlinked, not edited:\n{html}"
    );
}

#[test]
fn a_notes_detach_renders_when_given_a_callback() {
    let html = render(family_notes_detach);
    assert!(html.contains(">Detach<"), "the Detach affordance renders:\n{html}");
    assert!(
        html.contains(r#"aria-label="Detach N0007""#),
        "Detach carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        !html.contains("01920000-0000-7000-8000-0000000000a7"),
        "the attach assertion id is never rendered:\n{html}"
    );
}

#[test]
fn per_row_correction_never_renders_an_assertion_or_tag_id() {
    let html = render(family_view);
    for secret in [
        "01920000-0000-7000-8000-0000000000c3", // the child's assertion id
        "01920000-0000-7000-8000-0000000000e5", // the family event's assertion id
        "0190-secret-tag-id",                   // the tag's aggregate id
    ] {
        assert!(
            !html.contains(secret),
            "an aggregate/assertion id leaked into the HTML: {secret}\n{html}"
        );
    }
}
