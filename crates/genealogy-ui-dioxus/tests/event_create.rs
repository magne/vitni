//! SSR assertions for the Event create form (Phase 5 PR26): the draft header, the required Type
//! select, the place-mode select (none/existing/new §6b), and Save gated on dirty.

use dioxus::prelude::*;
use genealogy_ui::{EventDraft, EventPlaceKind, Localizer, ProvenanceDraft};
use genealogy_ui_dioxus::screens::{RecordActions, create_record_header, event_create_fields, provenance_block};

fn view(seed: EventDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let draft = use_signal(move || seed);
    let prov = use_signal(ProvenanceDraft::default);
    let can_save = draft().is_dirty();
    rsx! {
        {create_record_header(&loc.event_new_title(), &loc.record_draft_badge())}
        {event_create_fields(&loc, draft)}
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
    view(EventDraft::new())
}

fn new_place_view() -> Element {
    view(EventDraft {
        place_kind: EventPlaceKind::New,
        ..EventDraft::new()
    })
}

#[test]
fn create_pane_shows_the_type_and_place_selects() {
    let mut vdom = VirtualDom::new(empty_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    for needle in [
        "New event",
        "draft · not saved",
        "Type",
        r#"id="event-type""#,
        r#"id="event-place-kind""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert!(
        html.contains("disabled"),
        "Save disabled for a bare default draft:\n{html}"
    );
}

#[test]
fn a_new_place_selection_reveals_the_inline_place_fields() {
    let mut vdom = VirtualDom::new(new_place_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"id="event-new-place-name""#),
        "the inline new-place name field shows:\n{html}"
    );
    assert!(
        !html.contains("disabled"),
        "choosing a new place makes the draft dirty:\n{html}"
    );
}
