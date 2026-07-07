//! SSR assertions for the Place create form (Phase 5 PR26): the draft header, the required Type
//! select, the coordinate fields with `aria-invalid` + a field error on a bad/half-filled pair
//! (`record-editing.html` §7), and Save gated on dirty + valid.

use dioxus::prelude::*;
use genealogy_ui::{Localizer, PlaceDraft, ProvenanceDraft};
use genealogy_ui_dioxus::screens::{RecordActions, create_record_header, place_create_fields, provenance_block};

fn view(draft_seed: PlaceDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let draft = use_signal(move || draft_seed);
    let prov = use_signal(ProvenanceDraft::default);
    let can_save = draft().is_dirty() && draft().is_valid();
    rsx! {
        {create_record_header(&loc.place_new_title(), &loc.record_draft_badge(), rsx! {})}
        {place_create_fields(&loc, draft)}
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
