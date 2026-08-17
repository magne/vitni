//! SSR assertions for the DNA-match detail (Phase 5 PR27): the read-first Overview record (id · the
//! locked observed totals · the editable confirmation status), its edit mode swapping in the status
//! select while the observations render as disabled inputs (§3), the segments + shared-ancestors
//! tables, and the tags panel (name/colour, never id).

use dioxus::prelude::*;
use vitni_app::{ChromosomeSide, MatchStatus, TagRef, UsingKind};
use vitni_ui::{
    ActionLabel, AttachedRefVm, Category, ConfidenceLevel, DetailTab, DnaInferenceVm, DnaMatchDetail, DnaMatchDraft,
    DnaSegmentVm, Localizer, ProvenanceDraft, SharedAncestorVm, UsingRecordVm,
};
use vitni_ui_dioxus::screens::{
    DnaMatchEditForm, RecordActionLabels, RecordEditState, dna_match_ancestors_table, dna_match_overview,
    dna_match_segments_table, id_list, record_head_actions, tags_panel,
};
use vitni_ui_dioxus::shell::nav_state::NavState;

fn segments_tab() -> DetailTab {
    DetailTab {
        id: "segments",
        label: "Segments".to_owned(),
        count: None,
        action: Some(ActionLabel::AddSegment),
    }
}

fn ancestors_tab() -> DetailTab {
    DetailTab {
        id: "ancestors",
        label: "Shared ancestors".to_owned(),
        count: None,
        action: Some(ActionLabel::AddSharedAncestor),
    }
}

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
            side_kind: ChromosomeSide::Paternal,
            assertion_id: "01920000-0000-7000-8000-0000000000a1".to_owned(),
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
            assertion_id: "01920000-0000-7000-8000-0000000000a2".to_owned(),
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
        cited_by: vec![
            DnaInferenceVm {
                category: Category::People,
                human_id: "I0007".to_owned(),
                label: "John Smith".to_owned(),
                reading: "Half-sibling".to_owned(),
                confidence: Some(ConfidenceLevel::Normal),
                confidence_label: "Normal".to_owned(),
                source_count: 1,
            },
            DnaInferenceVm {
                category: Category::People,
                human_id: "I0007".to_owned(),
                label: "John Smith".to_owned(),
                reading: "Half-sibling (tree-supported)".to_owned(),
                confidence: Some(ConfidenceLevel::Low),
                confidence_label: "Low".to_owned(),
                source_count: 0,
            },
        ],
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
    // RecordLink (the cited-inference back-links) resolves NavState from context, so the harness
    // must provide it.
    use_context_provider(NavState::new);
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(false);
    let on_remove = use_callback(|_: (String, String)| {});
    let on_edit = use_callback(|_form: DnaMatchEditForm| {});
    let on_retract = use_callback(|_target: (String, String, bool)| {});
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (DnaMatchDraft, ProvenanceDraft)| {}))}
        {dna_match_overview(&loc, &detail, record)}
        {dna_match_segments_table(&loc, &segments_tab(), &detail.segments, on_edit, on_retract)}
        {dna_match_ancestors_table(&loc, &ancestors_tab(), &detail.shared_ancestors, on_edit, on_retract)}
        {id_list(&loc, &detail.notes, Some(on_retract))}
        {tags_panel(&loc, &detail.tags, on_remove)}
    }
}

/// The Segments / Shared-ancestors tabs with no rows — exercises the empty-state add affordance.
fn dna_match_empty_tabs() -> Element {
    let loc = loc();
    let on_edit = use_callback(|_form: DnaMatchEditForm| {});
    let on_retract = use_callback(|_target: (String, String, bool)| {});
    rsx! {
        {dna_match_segments_table(&loc, &segments_tab(), &[], on_edit, on_retract)}
        {dna_match_ancestors_table(&loc, &ancestors_tab(), &[], on_edit, on_retract)}
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
fn cited_inferences_render_reading_confidence_source_cue_and_back_link() {
    let html = render(dna_match_view);
    // The relationship reading and per-claim confidence badges for both inference rows.
    for needle in ["Half-sibling", "Half-sibling (tree-supported)", "Normal", "Low"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    // The cited-by list carries an accessible label, and each back-link a contextual one.
    assert!(
        html.contains(r#"aria-label="Cited by""#),
        "the cited-inference list carries an accessible label:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="View on John Smith""#),
        "the back-link carries a contextual accessible name:\n{html}"
    );
    // The back-link is a keyboard-operable control labelled with the citing record.
    assert!(
        html.contains("John Smith"),
        "the back-link labels the citing record:\n{html}"
    );
    // Source cue: one inference has a documentary source, the other flags none.
    assert!(
        html.contains("No source"),
        "the DNA-only inference flags no source:\n{html}"
    );
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

#[test]
fn segment_and_ancestor_rows_carry_edit_and_retract_with_row_scoped_labels() {
    let html = render(dna_match_view);
    assert!(
        html.contains(r#"aria-label="Edit 1""#),
        "the segment row Edit carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract 1""#),
        "the segment row Retract carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Edit Thomas Smith""#),
        "the shared-ancestor row Edit carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract Thomas Smith""#),
        "the shared-ancestor row Retract carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains("Retract this assertion — it stays in History"),
        "the Retract buttons carry the retract-title tooltip:\n{html}"
    );
}

#[test]
fn segment_and_ancestor_tabs_offer_add_triggers() {
    let html = render(dna_match_view);
    assert!(
        html.contains("+ Add segment"),
        "the Segments tab offers + Add segment:\n{html}"
    );
    assert!(
        html.contains("+ Link shared ancestor"),
        "the Shared-ancestors tab offers + Link shared ancestor:\n{html}"
    );
}

#[test]
fn empty_segment_and_ancestor_tabs_still_offer_add_triggers() {
    let html = render(dna_match_empty_tabs);
    assert!(
        html.contains("+ Add segment"),
        "an empty Segments tab keeps the add affordance:\n{html}"
    );
    assert!(
        html.contains("+ Link shared ancestor"),
        "an empty Shared-ancestors tab keeps the add affordance:\n{html}"
    );
}

#[test]
fn notes_carry_detach() {
    let html = render(dna_match_view);
    assert!(
        html.contains(r#"aria-label="Detach N0004""#),
        "the attached note carries a Detach:\n{html}"
    );
}

#[test]
fn no_assertion_id_is_ever_rendered() {
    let html = render(dna_match_view);
    for assertion_id in [
        "01920000-0000-7000-8000-0000000000a1",
        "01920000-0000-7000-8000-0000000000a2",
        "01920000-0000-7000-8000-0000000000d4",
    ] {
        assert!(
            !html.contains(assertion_id),
            "an assertion id must never be rendered ({assertion_id}):\n{html}"
        );
    }
}
