//! SSR assertions for the DNA-test detail (Phase 5 PR11): render the overview (kit metadata + tested
//! person), the haplogroups table, the matches table, and the tags panel. Asserts the kit fields, the
//! haplogroup, the match rows, and that a tag shows its name/colour but never its id.

use dioxus::prelude::*;
use genealogy_app::{TagRef, UsingKind};
use genealogy_ui::{DnaTestDetail, DnaTestMatchVm, Localizer, UsingRecordVm};
use genealogy_ui_dioxus::screens::{
    DnaTestEditForm, dna_test_haplogroups_table, dna_test_matches_table, dna_test_overview, dna_test_tags_panel,
};

/// A representative DNA test: `AncestryDNA` autosomal for John Smith, one haplogroup, one match, one tag.
fn sample() -> DnaTestDetail {
    DnaTestDetail {
        human_id: "D0002".to_owned(),
        id: "0190-test-2".to_owned(),
        title: "AncestryDNA — John Smith".to_owned(),
        provider: Some("AncestryDNA".to_owned()),
        test_type: Some("Autosomal".to_owned()),
        kit_id: Some("A1B2-C3D4".to_owned()),
        genome_build: Some("GRCh38".to_owned()),
        person: Some(UsingRecordVm {
            kind: UsingKind::Person,
            human_id: "I0042".to_owned(),
            id: "0190-person-42".to_owned(),
            label: "John Smith".to_owned(),
            kind_label: "Person".to_owned(),
        }),
        person_name: Some("John Smith".to_owned()),
        haplogroups: vec!["R-M269".to_owned()],
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
        notes: vec!["N0003".to_owned()],
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

/// Renders the overview, haplogroups, matches, and tags tabs together.
fn dna_test_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let editing = use_signal(|| None::<DnaTestEditForm>);
    let on_submit = use_callback(|_edit: genealogy_ui::DnaTestEdit| {});
    let detail = sample();
    rsx! {
        {dna_test_overview(&loc, &detail)}
        {dna_test_haplogroups_table(&loc, &detail.haplogroups)}
        {dna_test_matches_table(&loc, &detail.matches)}
        {dna_test_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

#[test]
fn overview_shows_kit_metadata_and_tested_person() {
    let mut vdom = VirtualDom::new(dna_test_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in ["AncestryDNA", "Autosomal", "A1B2-C3D4", "GRCh38", "John Smith"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn haplogroups_and_matches_are_listed() {
    let mut vdom = VirtualDom::new(dna_test_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in ["R-M269", "X0001", "D0005", "1750", "Aunt / Niece / Half-sib"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let mut vdom = VirtualDom::new(dna_test_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("Verified kit"), "tag name shown:\n{html}");
    assert!(html.contains("#74b449"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
