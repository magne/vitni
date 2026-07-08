//! SSR assertions for the DNA-test detail (Phase 5 PR27): the read-first Overview record (id · person
//! · provider · type · genome build · kit id), its edit mode swapping in inputs plus the sticky-header
//! Cancel/Save (person locked), the haplogroups + matches tables, and the tags panel (name/colour,
//! never id).

use dioxus::prelude::*;
use genealogy_app::{DnaGenomeBuild, DnaProvider, DnaTestType, TagRef, UsingKind};
use genealogy_ui::{
    AttachedRefVm, DnaTestDetail, DnaTestDraft, DnaTestMatchVm, HaplogroupRowVm, Localizer, ProvenanceDraft,
    UsingRecordVm,
};
use genealogy_ui_dioxus::screens::{
    DnaTestEditForm, RecordActionLabels, RecordEditState, dna_test_haplogroups_table, dna_test_matches_table,
    dna_test_overview, dna_test_tags_panel, id_list, record_head_actions,
};

/// A representative DNA test: `AncestryDNA` autosomal for John Smith, one haplogroup, one match, one tag.
fn sample() -> DnaTestDetail {
    DnaTestDetail {
        human_id: "D0002".to_owned(),
        id: "0190-test-2".to_owned(),
        title: "AncestryDNA — John Smith".to_owned(),
        provider: Some("AncestryDNA".to_owned()),
        provider_kind: Some(DnaProvider::AncestryDna),
        test_type: Some("Autosomal".to_owned()),
        test_type_kind: Some(DnaTestType::Autosomal),
        kit_id: Some("A1B2-C3D4".to_owned()),
        genome_build: Some("GRCh38".to_owned()),
        genome_build_kind: Some(DnaGenomeBuild::GRCh38),
        person: Some(UsingRecordVm {
            kind: UsingKind::Person,
            human_id: "I0042".to_owned(),
            id: "0190-person-42".to_owned(),
            label: "John Smith".to_owned(),
            kind_label: "Person".to_owned(),
        }),
        person_name: Some("John Smith".to_owned()),
        haplogroups: vec![HaplogroupRowVm {
            value: "R-M269".to_owned(),
            assertion_id: "0192-haplo-assert-1".to_owned(),
        }],
        matches: vec![DnaTestMatchVm {
            match_ref: UsingRecordVm {
                kind: UsingKind::DnaMatch,
                human_id: "X0001".to_owned(),
                id: "0190-match-1".to_owned(),
                label: "X0001".to_owned(),
                kind_label: "DNA match".to_owned(),
            },
            compared_test: Some(UsingRecordVm {
                kind: UsingKind::DnaTest,
                human_id: "D0005".to_owned(),
                id: "0190-test-5".to_owned(),
                label: "D0005".to_owned(),
                kind_label: "DNA test".to_owned(),
            }),
            shared_cm: Some("1750".to_owned()),
            percent_shared: Some("24.9".to_owned()),
            predicted: Some("Aunt / Niece / Half-sib".to_owned()),
        }],
        notes: vec![AttachedRefVm {
            human_id: "N0003".to_owned(),
            assertion_id: "01920000-0000-7000-8000-0000000000d3".to_owned(),
        }],
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Verified kit".to_owned(),
            color: Some("#74b449".to_owned()),
            priority: Some(3),
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

fn state(editing: bool) -> RecordEditState<DnaTestDraft> {
    let seed = DnaTestDraft::from_detail(&sample());
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

fn dna_test_view() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(false);
    let editing = use_signal(|| None::<DnaTestEditForm>);
    let on_submit = use_callback(|_edit: (genealogy_ui::DnaTestEdit, genealogy_ui::ProvenanceDraft)| {});
    let onedit = use_callback(|_: DnaTestEditForm| {});
    let onretract = use_callback(|_: (String, String, bool)| {});
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (DnaTestDraft, ProvenanceDraft)| {}))}
        {dna_test_overview(&loc, &detail, record)}
        {dna_test_haplogroups_table(&loc, &detail.haplogroups, onedit, onretract)}
        {dna_test_matches_table(&loc, &detail.matches)}
        {id_list(&loc, &detail.notes, Some(onretract))}
        {dna_test_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

/// Renders only the reverse-index Matches table (matches observed against this test) — it must carry
/// no per-row correction affordances.
fn dna_test_matches_view() -> Element {
    let loc = loc();
    let detail = sample();
    rsx! {
        {dna_test_matches_table(&loc, &detail.matches)}
    }
}

fn dna_test_edit() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(true);
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (DnaTestDraft, ProvenanceDraft)| {}))}
        {dna_test_overview(&loc, &detail, record)}
    }
}

#[test]
fn overview_is_read_first_with_an_edit_button_and_no_inputs() {
    let html = render(dna_test_view);
    assert!(html.contains(">Edit<"), "view mode offers Edit in the header:\n{html}");
    assert!(
        !html.contains("<input"),
        "view mode shows read boxes, not inputs:\n{html}"
    );
    for needle in ["AncestryDNA", "Autosomal", "A1B2-C3D4", "GRCh38", "John Smith"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn edit_mode_swaps_in_the_inputs_and_locks_the_person() {
    let html = render(dna_test_edit);
    assert!(html.contains("<input"), "edit mode swaps in inputs:\n{html}");
    assert!(
        html.contains("<select"),
        "edit mode swaps in the provider/type/build selects:\n{html}"
    );
    assert!(
        html.contains(">Cancel<") && html.contains(">Save<"),
        "Cancel/Save in the header:\n{html}"
    );
    assert!(
        html.contains(r#"id="dna-test-id""#),
        "the editable human id is present:\n{html}"
    );
    assert!(
        html.contains(r#"id="dna-test-person""#) && html.contains("disabled"),
        "the anchoring person renders as a locked (disabled) input:\n{html}"
    );
}

#[test]
fn haplogroups_and_matches_are_listed() {
    let html = render(dna_test_view);
    for needle in ["R-M269", "X0001", "D0005", "1750", "Aunt / Niece / Half-sib"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let html = render(dna_test_view);
    assert!(html.contains("Verified kit"), "tag name shown:\n{html}");
    assert!(html.contains("#74b449"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}

#[test]
fn haplogroup_rows_carry_edit_and_retract_with_row_scoped_labels() {
    let html = render(dna_test_view);
    assert!(
        html.contains(r#"aria-label="Edit R-M269""#),
        "the haplogroup row Edit carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Retract R-M269""#),
        "the haplogroup row Retract carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        html.contains("Retract this assertion — it stays in History"),
        "the Retract button carries the retract-title tooltip:\n{html}"
    );
}

#[test]
fn notes_carry_detach() {
    let html = render(dna_test_view);
    assert!(
        html.contains(r#"aria-label="Detach N0003""#),
        "the attached note carries a Detach:\n{html}"
    );
}

#[test]
fn reverse_index_matches_table_has_no_row_actions() {
    let html = render(dna_test_matches_view);
    assert!(html.contains("X0001"), "the observed match is still listed:\n{html}");
    assert!(
        !html.contains("row-actions"),
        "the reverse-index matches table carries no per-row correction cell:\n{html}"
    );
    assert!(
        !html.contains(">Edit<") && !html.contains(">Retract<"),
        "the reverse-index matches table offers no Edit/Retract:\n{html}"
    );
}

#[test]
fn no_assertion_id_is_ever_rendered() {
    let html = render(dna_test_view);
    for assertion_id in [
        "0192-haplo-assert-1",
        "01920000-0000-7000-8000-0000000000d3",
        "0190-secret-tag-id",
    ] {
        assert!(
            !html.contains(assertion_id),
            "assertion/tag id {assertion_id:?} must never be rendered:\n{html}"
        );
    }
}
