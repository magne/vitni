//! SSR assertions for the Repository create form (Phase 5 PR26): a "draft · not saved" header, the
//! Type/Name fields, the provenance block, and a Save/Cancel row with Save disabled while empty.

use dioxus::prelude::*;
use genealogy_ui::{Localizer, ProvenanceDraft, RepositoryDraft};
use genealogy_ui_dioxus::screens::{RecordActions, create_record_header, provenance_block, repository_create_fields};

fn create_view(dirty: bool) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let draft = use_signal(|| {
        if dirty {
            RepositoryDraft {
                name: "National Archives".to_owned(),
                ..RepositoryDraft::new()
            }
        } else {
            RepositoryDraft::new()
        }
    });
    let prov = use_signal(ProvenanceDraft::default);
    let can_save = draft().is_dirty();
    rsx! {
        {create_record_header(&loc.repository_new_title(), &loc.record_draft_badge(), rsx! {})}
        {repository_create_fields(&loc, draft)}
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
    create_view(false)
}

fn dirty_view() -> Element {
    create_view(true)
}

#[test]
fn create_pane_shows_the_draft_badge_and_labelled_fields() {
    let mut vdom = VirtualDom::new(empty_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    for needle in [
        "New repository",
        "draft · not saved",
        "Type",
        "Name",
        r#"id="repository-name""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn save_is_gated_on_dirty() {
    let mut empty = VirtualDom::new(empty_view);
    empty.rebuild_in_place();
    assert!(
        dioxus_ssr::render(&empty).contains("disabled"),
        "Save disabled while empty"
    );
    let mut dirty = VirtualDom::new(dirty_view);
    dirty.rebuild_in_place();
    assert!(
        !dioxus_ssr::render(&dirty).contains("disabled"),
        "Save enabled once dirty"
    );
}
