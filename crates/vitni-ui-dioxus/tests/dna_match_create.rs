//! SSR assertions for the DNA-match create pane (Phase 5 PR27/PR28): the shared record frame in create
//! mode — a "draft · not saved" header with Cancel/Save in the sticky head — plus the two existing-test
//! pickers, provider, and shared-cM, an aria-invalid flag on a bad shared-cM (rejected, never
//! zero-filled — §7), and Save gated on the required fields being present and every numeric parsing.

use dioxus::prelude::*;
use vitni_app::DnaProvider;
use vitni_ui::{DnaMatchDraft, Localizer, PickerSelection, PickerState, ProvenanceDraft, RowVm};
use vitni_ui_dioxus::components::{Button, ButtonVariant, PickerCallbacks, PickerConfig, PickerOptions, RecordPicker};
use vitni_ui_dioxus::screens::{
    RecordEditState, create_record_header, dna_match_create_fields, record_edit_provenance,
};

fn test_picker(name: &str) -> RecordPicker {
    RecordPicker {
        config: PickerConfig {
            label: "Test".to_owned(),
            name: name.to_owned(),
            entity_label: "DNA test".to_owned(),
            allow_new: false,
        },
        state: use_signal(PickerState::default),
        options: PickerOptions::Ready(vec![RowVm {
            id: "D0001".to_owned(),
            title: "Kit A".to_owned(),
            ..RowVm::default()
        }]),
        exclude: Vec::new(),
        callbacks: PickerCallbacks {
            onpick: Callback::new(|_: PickerSelection| {}),
            onclear: Callback::new(|()| {}),
            onnew: Callback::new(|_: String| {}),
        },
    }
}

fn view(seed: DnaMatchDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let record = RecordEditState::<DnaMatchDraft> {
        editing: use_signal(|| true),
        seed: use_signal(DnaMatchDraft::new),
        draft: use_signal(move || seed),
        prov: use_signal(ProvenanceDraft::default),
    };
    let test_a = test_picker("dna-match-test-a");
    let test_b = test_picker("dna-match-test-b");
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_label("save"), variant: ButtonVariant::Primary, small: true, disabled: !can_save, onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.dna_match_new_title(), &loc.record_draft_badge(), actions)}
        {dna_match_create_fields(&loc, record.draft, &test_a, &test_b)}
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

#[test]
fn the_two_tests_are_existing_record_pickers() {
    let html = render(complete_view);
    assert!(
        html.contains(r#"id="dna-match-test-a""#) && html.contains(r#"id="dna-match-test-b""#),
        "both test fields render as pickers:\n{html}"
    );
    assert!(
        html.contains(r#"placeholder="Find DNA test…""#),
        "the test fields search existing records instead of taking a free-text id:\n{html}"
    );
}
