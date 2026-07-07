//! SSR assertions for the DNA-test create pane (Phase 5 PR27): the shared record frame in create mode
//! — a "draft · not saved" header with Cancel/Save in the sticky head — plus the required Person field
//! (aria-invalid + error while blank, §7) and the provider/type/build selects. Save gated on the
//! person being present.

use dioxus::prelude::*;
use genealogy_ui::{DnaTestDraft, Localizer, ProvenanceDraft};
use genealogy_ui_dioxus::components::{Button, ButtonVariant};
use genealogy_ui_dioxus::screens::{
    RecordEditState, create_record_header, dna_test_create_fields, record_edit_provenance,
};

fn view(seed: DnaTestDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let record = RecordEditState::<DnaTestDraft> {
        editing: use_signal(|| true),
        seed: use_signal(DnaTestDraft::new),
        draft: use_signal(move || seed),
        prov: use_signal(ProvenanceDraft::default),
    };
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_label("save"), variant: ButtonVariant::Primary, small: true, disabled: !can_save, onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.dna_test_new_title(), &loc.record_draft_badge(), actions)}
        {dna_test_create_fields(&loc, record.draft)}
        {record_edit_provenance(&loc, record)}
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

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn a_blank_person_is_flagged_and_blocks_save() {
    let html = render(empty_view);
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
    let html = render(with_person_view);
    assert!(
        !html.contains(r#"aria-invalid="true""#),
        "no invalid flag with a person:\n{html}"
    );
    assert!(!html.contains("disabled"), "Save is enabled with a person:\n{html}");
}
