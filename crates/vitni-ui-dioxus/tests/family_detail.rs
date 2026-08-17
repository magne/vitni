//! SSR assertions for the Family detail (Phase 5 PR27): the read-first Overview (Partners + Marriage
//! cards with the evidence-first cues), its edit mode swapping in the family's only scalar — the
//! editable id — plus the sticky-header Cancel/Save, the children + events tables, and the tags panel
//! (name/colour, never id).

use dioxus::prelude::*;
use vitni_app::{ChildParentRelationship, Rect, TagRef};
use vitni_ui::{
    ChildRelationshipVm, CitationRefVm, ConfidenceLevel, EvidenceAxis, EvidenceAxisVm, FamilyChildVm, FamilyDetail,
    FamilyDraft, FamilyEventVm, Localizer, MediaRefVm, PartnerVm, ProvenanceDraft,
};
use vitni_ui_dioxus::screens::{
    ChildRemoval, FamilyEditForm, MediaTabState, RecordActionLabels, RecordEditState, child_removal_side_panel,
    citations_table, family_children_table, family_events_table, family_overview, media_tab, note_cards,
    record_head_actions, tags_panel,
};
use vitni_ui_dioxus::shell::nav_state::NavState;

/// A representative marriage-register citation, used to back the partner + marriage provenance cues.
fn marriage_citation() -> CitationRefVm {
    CitationRefVm {
        human_id: "C0001".to_owned(),
        source: Some("Trinity Church marriage register".to_owned()),
        source_id: Some("S0003".to_owned()),
        page: Some("vol. 5, f. 18".to_owned()),
        backs_count: 0,
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
            confidence: Some(ConfidenceLevel::High),
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
                ChildRelationshipVm {
                    partner_human_id: "I0001".to_owned(),
                    label: "Birth".to_owned(),
                    kind: ChildParentRelationship::Birth,
                    assertion_id: "01920000-0000-7000-8000-0000000000d1".to_owned(),
                    source_count: 1,
                },
                ChildRelationshipVm {
                    partner_human_id: "I0002".to_owned(),
                    label: "Step".to_owned(),
                    kind: ChildParentRelationship::Step,
                    assertion_id: "01920000-0000-7000-8000-0000000000d2".to_owned(),
                    source_count: 0,
                },
            ],
            confidence: Some(ConfidenceLevel::Normal),
            confidence_label: "Normal".to_owned(),
            source_count: 0,
            assertion_id: "01920000-0000-7000-8000-0000000000c3".to_owned(),
        }],
        events: vec![FamilyEventVm {
            human_id: "E0001".to_owned(),
            type_label: "Marriage".to_owned(),
            date: Some("14 Jun 1876".to_owned()),
            place: Some("Trinity Church, New York".to_owned()),
            confidence: Some(ConfidenceLevel::High),
            confidence_label: "High".to_owned(),
            source_count: 1,
            citations: Vec::new(),
            assertion_id: "01920000-0000-7000-8000-0000000000e5".to_owned(),
        }],
        citations: vec![CitationRefVm {
            human_id: "C0001".to_owned(),
            source: Some("Trinity Church marriage register".to_owned()),
            source_id: Some("S0003".to_owned()),
            page: Some("vol. 5, f. 18".to_owned()),
            backs_count: 3,
            confidence: Some(ConfidenceLevel::High),
            confidence_label: Some("High".to_owned()),
            evidence_axes: vec![EvidenceAxisVm {
                axis: EvidenceAxis::Source,
                label: "Original".to_owned(),
            }],
            asserted_by: Some("asserted by magne · 2026-06-21 16:05".to_owned()),
            assertion_id: Some("01920000-0000-7000-8000-0000000000c9".to_owned()),
        }],
        media: sample_media(),
        notes: Vec::new(),
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Ancestral line".to_owned(),
            color: Some("#74b449".to_owned()),
            priority: Some(1),
        }],
        restrictions: Vec::new(),
        research_notes: Vec::new(),
        history: Vec::new(),
    }
}

/// The family's attached media (one captioned wedding portrait ref).
fn sample_media() -> Vec<MediaRefVm> {
    vec![MediaRefVm {
        human_id: "O0001".to_owned(),
        caption: Some("Wedding portrait, 1876".to_owned()),
        crop: None,
        path: None,
        mime: None,
        assertion_id: "01920000-0000-7000-8000-0000000000f1".to_owned(),
    }]
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
    let on_remove = use_callback(|_: (String, String)| {});
    let on_retract = use_callback(|_target: (String, String, bool)| {});
    let on_child_remove = use_callback(|_child: ChildRemoval| {});
    let on_edit_open = use_callback(|_form: FamilyEditForm| {});
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (FamilyDraft, ProvenanceDraft)| {}))}
        {family_overview(&loc, &detail, editing, record, on_retract)}
        {family_children_table(&loc, &detail, on_edit_open, on_retract, on_child_remove)}
        {family_events_table(&loc, &detail.events, on_retract)}
        {tags_panel(&loc, &detail.tags, on_remove)}
    }
}

/// The Children tab's removal confirm, armed for the sample child — the "record a change" panel that
/// dispatches `FamilyEdit::RemoveChild` (as opposed to the shared Retract panel, which corrects).
fn child_removal_panel_view() -> Element {
    let loc = loc();
    let armed = use_signal(|| {
        Some(ChildRemoval {
            human_id: "I0003".to_owned(),
            label: "Jonathan Smith".to_owned(),
        })
    });
    let reason = use_signal(String::new);
    rsx! {
        {child_removal_side_panel(&loc, armed, reason, use_callback(|()| {}))}
    }
}

/// The family Citations tab: the shared citations table with the plural-subject "Backs" column and a
/// per-row Detach action.
fn family_citations_view() -> Element {
    use_context_provider(NavState::new);
    let loc = loc();
    let on_retract = use_callback(|_target: (String, String, bool)| {});
    let detail = sample();
    rsx! {
        {citations_table::<FamilyEditForm>(&loc, &detail.citations, true, on_retract)}
    }
}

#[test]
fn family_citations_tab_shows_the_canonical_shape_with_backs_and_detach() {
    let html = render(family_citations_view);
    for needle in [
        ">Source<",
        ">Page<",
        ">Backs<",
        ">Confidence<",
        ">Evidence<",
        "Trinity Church marriage register",
        "vol. 5, f. 18",
        ">3<",
    ] {
        assert!(
            html.contains(needle),
            "expected {needle:?} in the family citations table:\n{html}"
        );
    }
    assert!(
        html.contains(r#"aria-label="Detach C0001""#),
        "each citation row carries a labelled Detach action:\n{html}"
    );
}

/// A notes list with an armed detach callback — exercises the attachment Detach affordance.
fn family_notes_detach() -> Element {
    let loc = loc();
    let on_retract = use_callback(|_target: (String, String, bool)| {});
    let notes = vec![vitni_ui::AttachedRefVm {
        human_id: "N0007".to_owned(),
        note_type: Some(vitni_app::NoteType::Research),
        type_label: Some("Research".to_owned()),
        text: Some("Marriage banns suggest a betrothal in late 1875; check the parish bulletin.".to_owned()),
        language: Some("en".to_owned()),
        assertion_id: "01920000-0000-7000-8000-0000000000a7".to_owned(),
    }];
    rsx! {
        {note_cards(&loc, &notes, Some(on_retract))}
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
fn children_rows_offer_edit_remove_and_retract_as_distinct_verbs() {
    let html = render(family_view);
    // Edit opens the child form pre-filled; Remove ends the membership (`ChildRemoved`); Retract
    // withdraws the membership claim itself. All three keep the log intact — they differ in meaning.
    assert!(html.contains(">Edit<"), "the visible Edit verb:\n{html}");
    assert!(html.contains(">Remove<"), "the visible Remove verb:\n{html}");
    assert!(html.contains(">Retract<"), "the visible Retract verb:\n{html}");
    for needle in [
        r#"aria-label="Edit Jonathan Smith""#,
        r#"aria-label="Remove Jonathan Smith""#,
        r#"aria-label="Retract Jonathan Smith""#,
    ] {
        assert!(
            html.contains(needle),
            "each child action carries a row-scoped accessible name, expected {needle:?}:\n{html}"
        );
    }
    // The tooltips separate recording a change from correcting a mistake (prefix-matched, so the
    // sentence can carry punctuation the HTML escapes).
    assert!(
        html.contains(r#"title="Remove this child from the family"#),
        "Remove says the membership ended:\n{html}"
    );
    assert!(
        html.contains(r#"title="Retract this child"#),
        "Retract says the claim was recorded in error:\n{html}"
    );
}

#[test]
fn the_child_removal_panel_records_a_change_and_never_says_retract() {
    let html = render(child_removal_panel_view);
    assert!(
        html.contains("Remove from family"),
        "the panel is titled for the membership change:\n{html}"
    );
    assert!(
        html.contains("Jonathan Smith"),
        "the panel names the child being removed:\n{html}"
    );
    assert!(
        html.contains("The removal is recorded in History"),
        "the panel says the removal is logged, not destructive:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Remove Jonathan Smith""#),
        "the confirm carries the row-scoped accessible name:\n{html}"
    );
    assert!(
        !html.contains("Retract") && !html.contains("retract"),
        "a removal panel never offers to retract the claim:\n{html}"
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
        "01920000-0000-7000-8000-0000000000c3", // the child's membership assertion id
        "01920000-0000-7000-8000-0000000000d1", // the child–P1 relationship assertion id
        "01920000-0000-7000-8000-0000000000d2", // the child–P2 relationship assertion id
        "01920000-0000-7000-8000-0000000000e5", // the family event's assertion id
        "0190-secret-tag-id",                   // the tag's aggregate id
    ] {
        assert!(
            !html.contains(secret),
            "an aggregate/assertion id leaked into the HTML: {secret}\n{html}"
        );
    }
}

/// The Family Media tab (issue #199): the gallery card opens the shared crop viewer, mirroring the
/// Person screen's `media_tab` wiring (`family.rs`'s "media" tab arm).
fn family_media_tab_view() -> Element {
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
    let html = render(family_media_tab_view);
    assert!(
        html.contains("media-open"),
        "the gallery card opens the crop viewer (ADR 0017 §GUI):\n{html}"
    );
    assert!(
        html.contains("Set region") && html.contains("Clear region"),
        "the crop viewer overlay renders with its Set/Clear region actions:\n{html}"
    );
}
