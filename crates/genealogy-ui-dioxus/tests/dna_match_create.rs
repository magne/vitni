//! SSR assertions for the DNA-match create form (Phase 5 PR26): the draft header, the two tests +
//! provider + shared-cM, an aria-invalid flag on a bad shared-cM (rejected, never zero-filled — §7),
//! and Save gated on the required fields being present and every numeric parsing.

use dioxus::prelude::*;
use genealogy_app::DnaProvider;
use genealogy_ui::{DnaMatchDraft, Localizer, ProvenanceDraft};
use genealogy_ui_dioxus::screens::{RecordActions, create_record_header, dna_match_create_fields, provenance_block};

fn view(seed: DnaMatchDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let draft = use_signal(move || seed);
    let prov = use_signal(ProvenanceDraft::default);
    let can_save = draft().is_dirty() && draft().is_valid();
    rsx! {
        {create_record_header(&loc.dna_match_new_title(), &loc.record_draft_badge(), rsx! {})}
        {dna_match_create_fields(&loc, draft)}
        {provenance_block(&loc, prov)}
        RecordActions {
            save_label: loc.action_label("save"),
            cancel_label: loc.action_label("cancel"),
            can_save,
            onsave: move |()| {},
            oncancel: move |()| {},
        }
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

#[test]
fn a_bad_shared_cm_is_flagged_and_blocks_save() {
    let mut vdom = VirtualDom::new(bad_shared_cm_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
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
    let mut vdom = VirtualDom::new(complete_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        !html.contains(r#"aria-invalid="true""#),
        "no invalid flag when parseable:\n{html}"
    );
    assert!(
        !html.contains("disabled"),
        "Save enabled for a complete valid draft:\n{html}"
    );
}
