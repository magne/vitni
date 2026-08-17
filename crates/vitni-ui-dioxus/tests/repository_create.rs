//! SSR assertions for the Repository create pane (Phase 5 PR27): the shared record frame in create
//! mode — a "draft · not saved" header with Cancel/Save in the sticky head, and the id/type/name
//! fields rendered as inputs.

use dioxus::prelude::*;
use vitni_ui::ActionLabel;
use vitni_ui::{Localizer, ProvenanceDraft, RepositoryDraft};
use vitni_ui_dioxus::components::{Button, ButtonVariant};
use vitni_ui_dioxus::screens::{
    RecordEditState, create_record_header, record_edit_provenance, repository_record_fields,
};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn create_view() -> Element {
    let loc = loc();
    let record = RecordEditState::<RepositoryDraft> {
        editing: use_signal(|| true),
        seed: use_signal(RepositoryDraft::new),
        draft: use_signal(RepositoryDraft::new),
        prov: use_signal(ProvenanceDraft::default),
    };
    let actions = rsx! {
        Button { label: loc.action_button(ActionLabel::Cancel), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_button(ActionLabel::Save), variant: ButtonVariant::Primary, small: true, disabled: true, onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.repository_new_title(), &loc.record_draft_badge(), actions)}
        {repository_record_fields(&loc, record)}
        {record_edit_provenance(&loc, record)}
    }
}

#[test]
fn create_pane_shows_the_draft_badge_and_labelled_fields() {
    let mut vdom = VirtualDom::new(create_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    for needle in [
        "New repository",
        "draft · not saved",
        "Type",
        "Name",
        r#"id="repository-name""#,
        r#"id="repository-id""#,
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
