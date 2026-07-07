//! SSR assertions for the DNA-test create form (Phase 5 PR26): the draft header, the required Person
//! field (aria-invalid + error while blank, §7), the provider/type/build selects, and Save gated on
//! the person being present.

use dioxus::prelude::*;
use genealogy_ui::{DnaTestDraft, Localizer, ProvenanceDraft};
use genealogy_ui_dioxus::screens::{RecordActions, create_record_header, dna_test_create_fields, provenance_block};

fn view(seed: DnaTestDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let draft = use_signal(move || seed);
    let prov = use_signal(ProvenanceDraft::default);
    let can_save = draft().is_dirty() && draft().is_valid();
    rsx! {
        {create_record_header(&loc.dna_test_new_title(), &loc.record_draft_badge(), rsx! {})}
        {dna_test_create_fields(&loc, draft)}
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

fn empty_view() -> Element {
    view(DnaTestDraft::new())
}

fn with_person_view() -> Element {
    view(DnaTestDraft {
        person: "I0001".to_owned(),
        ..DnaTestDraft::new()
    })
}

#[test]
fn a_blank_person_is_flagged_and_blocks_save() {
    let mut vdom = VirtualDom::new(empty_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    for needle in ["New DNA test", "draft · not saved", "Person", r#"id="dna-test-person""#] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert!(
        html.contains(r#"aria-invalid="true""#),
        "the blank person is flagged:\n{html}"
    );
    assert!(html.contains("A person is required"), "the field error shows:\n{html}");
    assert!(html.contains("disabled"), "Save is blocked without a person:\n{html}");
}

#[test]
fn a_person_enables_save() {
    let mut vdom = VirtualDom::new(with_person_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        !html.contains(r#"aria-invalid="true""#),
        "no invalid flag with a person:\n{html}"
    );
    assert!(!html.contains("disabled"), "Save is enabled with a person:\n{html}");
}
