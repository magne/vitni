//! SSR-probe coverage for #207: a successful create must bump [`NavState::data_version`] so the
//! Explorer list and rail counts refetch, mirroring the edit path's `finish_record_save`. A failed
//! create must not — a failed write changed nothing, so claiming a data change would send every
//! `data_version` subscriber off to refetch for no reason.
//!
//! Same host-free style as `edit_stash.rs`: `finish_draft_commit` is a free fn, so it is driven
//! directly from a mount-time hook over a bare [`NavState`], with the version rendered as a marker.

use dioxus::prelude::*;
use genealogy_ui::Category;
use genealogy_ui_dioxus::screens::{DraftCommit, finish_draft_commit};
use genealogy_ui_dioxus::shell::nav_state::NavState;

/// The shell-level marker block: the data-change ticket and the shell notice, if any.
fn probe(nav: &NavState) -> Element {
    let version = *nav.data_version.read();
    let notice = nav
        .notice
        .read()
        .as_ref()
        .map_or_else(|| "NONE".to_owned(), |notice| notice.message.clone());
    rsx! {
        div { "VERSION:{version}" }
        div { "NOTICE:{notice}" }
    }
}

fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn create_ok() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let draft_id = nav.open_create(Category::People);
        finish_draft_commit(
            Ok("P0001".to_owned()),
            DraftCommit {
                category: Category::People,
                draft_id,
                label: None,
                created: "Created".to_owned(),
            },
            nav,
        );
    });
    probe(&nav)
}

#[test]
fn a_successful_create_bumps_the_data_version() {
    let html = render(create_ok);
    assert!(
        html.contains("VERSION:1"),
        "a committed draft marks the workspace changed, same as an edit save:\n{html}"
    );
}

#[test]
fn a_successful_create_leaves_the_created_notice() {
    // Create had no feedback for a completed action at all (#208) — the shell notice is the fix.
    let html = render(create_ok);
    assert!(
        html.contains("NOTICE:Created"),
        "a successful create surfaces a confirmation, same as an edit save:\n{html}"
    );
}

fn create_err() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        let draft_id = nav.open_create(Category::People);
        finish_draft_commit(
            Err("could not save".to_owned()),
            DraftCommit {
                category: Category::People,
                draft_id,
                label: None,
                created: "Created".to_owned(),
            },
            nav,
        );
    });
    probe(&nav)
}

#[test]
fn a_failed_create_does_not_bump_the_data_version_but_still_notifies() {
    let html = render(create_err);
    assert!(
        html.contains("VERSION:0"),
        "nothing was written, so no subscriber should refetch:\n{html}"
    );
    assert!(
        html.contains("NOTICE:could not save"),
        "the failure is still surfaced as a shell notice:\n{html}"
    );
}
