//! SSR assertions for the DNA-match create pane (Phase 5 PR27): the shared record frame in create mode
//! — a "draft · not saved" header with Cancel/Save in the sticky head — plus the two tests, provider,
//! and shared-cM, an aria-invalid flag on a bad shared-cM (rejected, never zero-filled — §7), and Save
//! gated on the required fields being present and every numeric parsing.

use dioxus::prelude::*;
use genealogy_app::DnaProvider;
use genealogy_ui::{DnaMatchDraft, Localizer, ProvenanceDraft};
use genealogy_ui_dioxus::components::{Button, ButtonVariant};
use genealogy_ui_dioxus::screens::{
    RecordEditState, create_record_header, dna_match_create_fields, record_edit_provenance,
};

fn view(seed: DnaMatchDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let record = RecordEditState::<DnaMatchDraft> {
        editing: use_signal(|| true),
        seed: use_signal(DnaMatchDraft::new),
        draft: use_signal(move || seed),
        prov: use_signal(ProvenanceDraft::default),
    };
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_label("save"), variant: ButtonVariant::Primary, small: true, disabled: !can_save, onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.dna_match_new_title(), &loc.record_draft_badge(), actions)}
        {dna_match_create_fields(&loc, record.draft)}
        {record_edit_provenance(&loc, record)}
    }
}

fn complete() -> DnaMatchDraft {
    DnaMatchDraft {
        test_a: "D0001".to_owned(),
        test_b: "D0002".to_owned(),
        provider: Some(DnaProvider::AncestryDna),
        shared_cm: "1200".to_owned(),
        ..DnaMatchDraft::new()
    }
}

fn bad_shared_cm_view() -> Element {
    view(DnaMatchDraft {
        shared_cm: "lots".to_owned(),
        ..complete()
    })
}

fn complete_view() -> Element {
    view(complete())
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn a_bad_shared_cm_is_flagged_and_blocks_save() {
    let html = render(bad_shared_cm_view);
    for needle in ["New DNA match", "draft · not saved", r#"id="dna-match-shared-cm""#] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert!(
        html.contains(r#"aria-invalid="true""#),
        "the bad shared-cM is flagged:\n{html}"
    );
    assert!(
        html.contains("Enter a valid centimorgan"),
        "the field error shows:\n{html}"
    );
    assert!(
        html.contains("disabled"),
        "Save blocked while shared-cM is unparseable:\n{html}"
    );
}

#[test]
fn a_complete_draft_enables_save() {
    let html = render(complete_view);
    assert!(
        !html.contains(r#"aria-invalid="true""#),
        "no invalid flag when parseable:\n{html}"
    );
    assert!(
        !html.contains("disabled"),
        "Save enabled for a complete valid draft:\n{html}"
    );
}
