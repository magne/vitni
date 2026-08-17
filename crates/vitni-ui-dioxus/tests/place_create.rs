//! SSR assertions for the Place create pane (Phase 5 PR27): the shared record frame in create mode —
//! the draft header with Cancel/Save, the required Type select, the coordinate fields flagging an
//! invalid/half-filled pair (`record-editing.html` §7), and Save gated on dirty + valid.

use dioxus::prelude::*;
use vitni_ui::ActionLabel;
use vitni_ui::{Localizer, PlaceDraft, ProvenanceDraft};
use vitni_ui_dioxus::components::{Button, ButtonVariant, Input};
use vitni_ui_dioxus::screens::{RecordEditState, create_record_header, place_record_fields, record_edit_provenance};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn view(draft_seed: PlaceDraft) -> Element {
    let loc = loc();
    let record = RecordEditState::<PlaceDraft> {
        editing: use_signal(|| true),
        seed: use_signal(PlaceDraft::new),
        draft: use_signal(move || draft_seed),
        prov: use_signal(ProvenanceDraft::default),
    };
    let mut draft = record.draft;
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_button(ActionLabel::Cancel), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_button(ActionLabel::Save), variant: ButtonVariant::Primary, small: true, disabled: !can_save, onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.place_new_title(), &loc.record_draft_badge(), actions)}
        {place_record_fields(&loc, record, None)}
        Input {
            label: loc.field_label("name"),
            name: "place-name".to_owned(),
            value: draft().name.clone(),
            oninput: move |event: FormEvent| draft.write().name = event.value(),
        }
        {record_edit_provenance(&loc, record)}
    }
}

fn valid_view() -> Element {
    view(PlaceDraft {
        name: "Oslo".to_owned(),
        ..PlaceDraft::new()
    })
}

fn bad_latitude_view() -> Element {
    view(PlaceDraft {
        latitude: "not-a-number".to_owned(),
        longitude: "10.75".to_owned(),
        ..PlaceDraft::new()
    })
}

#[test]
fn create_pane_shows_the_draft_badge_and_type_and_coordinate_fields() {
    let mut vdom = VirtualDom::new(valid_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    for needle in [
        "New place",
        "draft · not saved",
        "Type",
        "Latitude",
        "Longitude",
        r#"id="place-latitude""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert!(
        !html.contains("disabled"),
        "Save is enabled for a valid, named draft:\n{html}"
    );
}

#[test]
fn a_bad_latitude_is_flagged_and_blocks_save() {
    let mut vdom = VirtualDom::new(bad_latitude_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"aria-invalid="true""#),
        "the bad latitude carries aria-invalid:\n{html}"
    );
    assert!(
        html.contains("Enter a valid coordinate"),
        "the field error shows:\n{html}"
    );
    assert!(
        html.contains("disabled"),
        "Save is blocked while a coordinate is invalid:\n{html}"
    );
}
