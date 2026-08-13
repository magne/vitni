//! SSR assertions for the Event create pane (Phase 5 PR27/PR28): the shared record frame in create
//! mode — a "draft · not saved" header with Cancel/Save in the sticky head — plus the required Type
//! select and the find-or-create Place picker (existing → a chip; "+ New" → an inline place draft
//! card). Save gated on dirty.

use dioxus::prelude::*;
use vitni_ui::{EventDraft, Localizer, NewPlaceFields, PickerSelection, PickerState, ProvenanceDraft, RecordLink};
use vitni_ui_dioxus::components::{Button, ButtonVariant, PickerCallbacks, PickerConfig, PickerOptions, RecordPicker};
use vitni_ui_dioxus::screens::{RecordEditState, create_record_header, event_create_fields, record_edit_provenance};

fn noop_callbacks() -> PickerCallbacks {
    PickerCallbacks {
        onpick: Callback::new(|_: PickerSelection| {}),
        onclear: Callback::new(|()| {}),
        onnew: Callback::new(|_: String| {}),
    }
}

fn view(seed: EventDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let record = RecordEditState::<EventDraft> {
        editing: use_signal(|| true),
        seed: use_signal(EventDraft::new),
        draft: use_signal(move || seed),
        prov: use_signal(ProvenanceDraft::default),
    };
    let place = RecordPicker {
        config: PickerConfig {
            label: loc.field_label("place"),
            name: "event-place".to_owned(),
            entity_label: "place".to_owned(),
            allow_new: true,
        },
        state: use_signal(PickerState::default),
        options: PickerOptions::Ready(Vec::new()),
        exclude: Vec::new(),
        callbacks: noop_callbacks(),
    };
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_label("save"), variant: ButtonVariant::Primary, small: true, disabled: !can_save, onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.event_new_title(), &loc.record_draft_badge(), actions)}
        {event_create_fields(&loc, record.draft, &place)}
        {record_edit_provenance(&loc, record)}
    }
}

fn empty_view() -> Element {
    view(EventDraft::new())
}

fn new_place_view() -> Element {
    view(EventDraft {
        place: RecordLink::New(NewPlaceFields::default()),
        ..EventDraft::new()
    })
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn create_pane_shows_the_type_select_and_place_picker() {
    let html = render(empty_view);
    for needle in [
        "New event",
        "draft · not saved",
        "Type",
        r#"id="event-type""#,
        r#"for="event-date""#,
        r#"id="event-place""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert!(
        html.contains("disabled"),
        "Save disabled for a bare default draft:\n{html}"
    );
}

#[test]
fn a_new_place_selection_reveals_the_inline_place_draft_card() {
    let html = render(new_place_view);
    assert!(
        html.contains(r#"id="event-new-place-name""#),
        "the inline new-place name field shows:\n{html}"
    );
    assert!(
        html.contains(r#"class="draft-card""#),
        "the inline new place renders in a draft card:\n{html}"
    );
    assert!(
        !html.contains("disabled"),
        "choosing a new place makes the draft dirty:\n{html}"
    );
}
