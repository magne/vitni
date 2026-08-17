//! SSR assertions for the DNA-test create pane (Phase 5 PR27/PR28): the shared record frame in create
//! mode — a "draft · not saved" header with Cancel/Save in the sticky head — plus the required Person
//! field, now an existing-person picker (a required-field error while unpicked, §7) and the
//! provider/type/build selects. Save gated on the person being present.

use dioxus::prelude::*;
use vitni_ui::ActionLabel;
use vitni_ui::{DnaTestDraft, Localizer, PickerSelection, PickerState, ProvenanceDraft};
use vitni_ui_dioxus::components::{Button, ButtonVariant, PickerCallbacks, PickerConfig, PickerOptions, RecordPicker};
use vitni_ui_dioxus::screens::{RecordEditState, create_record_header, dna_test_create_fields, record_edit_provenance};

fn person_picker() -> RecordPicker {
    RecordPicker {
        config: PickerConfig {
            label: "Person".to_owned(),
            name: "dna-test-person".to_owned(),
            entity_label: "person".to_owned(),
            allow_new: false,
        },
        state: use_signal(PickerState::default),
        options: PickerOptions::Ready(Vec::new()),
        exclude: Vec::new(),
        callbacks: PickerCallbacks {
            onpick: Callback::new(|_: PickerSelection| {}),
            onclear: Callback::new(|()| {}),
            onnew: Callback::new(|_: String| {}),
        },
    }
}

fn view(seed: DnaTestDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let record = RecordEditState::<DnaTestDraft> {
        editing: use_signal(|| true),
        seed: use_signal(DnaTestDraft::new),
        draft: use_signal(move || seed),
        prov: use_signal(ProvenanceDraft::default),
    };
    let person = person_picker();
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_button(ActionLabel::Cancel), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_button(ActionLabel::Save), variant: ButtonVariant::Primary, small: true, disabled: !can_save, onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.dna_test_new_title(), &loc.record_draft_badge(), actions)}
        {dna_test_create_fields(&loc, record.draft, &person)}
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
        html.contains(r#"placeholder="Find person…""#),
        "the person field is an existing-record picker, not a free-text id:\n{html}"
    );
    assert!(html.contains("A person is required"), "the field error shows:\n{html}");
    assert!(html.contains("disabled"), "Save is blocked without a person:\n{html}");
}

#[test]
fn a_person_enables_save() {
    let html = render(with_person_view);
    assert!(
        !html.contains("A person is required"),
        "no required error once a person is set:\n{html}"
    );
    assert!(!html.contains("disabled"), "Save is enabled with a person:\n{html}");
}
