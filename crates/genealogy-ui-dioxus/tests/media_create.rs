//! SSR assertions for the Media create pane (Phase 5 PR27): the shared record frame in create mode —
//! a "draft · not saved" header with Cancel/Save in the sticky head, and the id/paths/MIME/date fields
//! rendered editable (checksum locked).

use dioxus::prelude::*;
use genealogy_ui::{Localizer, MediaDraft, ProvenanceDraft};
use genealogy_ui_dioxus::components::{Button, ButtonVariant};
use genealogy_ui_dioxus::screens::{
    RecordEditState, create_record_header, media_record_fields, record_edit_provenance,
};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn create_view() -> Element {
    let loc = loc();
    let record = RecordEditState::<MediaDraft> {
        editing: use_signal(|| true),
        seed: use_signal(MediaDraft::new),
        draft: use_signal(MediaDraft::new),
        prov: use_signal(ProvenanceDraft::default),
    };
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_label("save"), variant: ButtonVariant::Primary, small: true, disabled: true, onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.media_new_title(), &loc.record_draft_badge(), actions)}
        {media_record_fields(&loc, record)}
        {record_edit_provenance(&loc, record)}
    }
}

#[test]
fn create_pane_shows_the_draft_badge_and_labelled_fields() {
    let mut vdom = VirtualDom::new(create_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    for needle in [
        "New media",
        "draft · not saved",
        "File path",
        "Web path",
        "MIME",
        r#"id="media-file-path""#,
        r#"id="media-id""#,
        r#"for="media-date""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn create_pane_offers_cancel_and_save_in_the_head() {
    let mut vdom = VirtualDom::new(create_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"class="head-actions""#),
        "Cancel/Save sit in the head-actions:\n{html}"
    );
    assert!(
        html.contains(">Cancel<") && html.contains(">Save<"),
        "both actions render:\n{html}"
    );
}
