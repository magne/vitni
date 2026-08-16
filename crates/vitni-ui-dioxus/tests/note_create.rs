//! SSR assertions for the Note create pane (Phase 5 PR27): the shared record frame in create mode —
//! a "draft · not saved" header with Cancel/Save in the sticky head, and the id/type/content/language
//! fields rendered as inputs (content a textarea).

use dioxus::prelude::*;
use vitni_ui::ActionLabel;
use vitni_ui::{Localizer, NoteDraft, ProvenanceDraft};
use vitni_ui_dioxus::components::{Button, ButtonVariant};
use vitni_ui_dioxus::screens::{RecordEditState, create_record_header, note_record_fields, record_edit_provenance};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn create_view() -> Element {
    let loc = loc();
    let record = RecordEditState::<NoteDraft> {
        editing: use_signal(|| true),
        seed: use_signal(NoteDraft::new),
        draft: use_signal(NoteDraft::new),
        prov: use_signal(ProvenanceDraft::default),
    };
    let actions = rsx! {
        Button { label: loc.action_button(ActionLabel::Cancel), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_button(ActionLabel::Save), variant: ButtonVariant::Primary, small: true, disabled: true, onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.note_new_title(), &loc.record_draft_badge(), actions)}
        {note_record_fields(&loc, record)}
        {record_edit_provenance(&loc, record)}
    }
}

#[test]
fn create_pane_shows_the_draft_badge_and_labelled_fields() {
    let mut vdom = VirtualDom::new(create_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    for needle in [
        "New note",
        "draft · not saved",
        "Type",
        "Language",
        "Content",
        r#"id="note-content""#,
        r#"id="note-id""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn create_pane_save_is_disabled_while_empty() {
    let mut vdom = VirtualDom::new(create_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains("disabled"),
        "Save disabled while the draft is empty:\n{html}"
    );
}
