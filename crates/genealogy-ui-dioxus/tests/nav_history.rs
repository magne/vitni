//! SSR-probe assertions for back/forward navigation history on [`NavState`] (nav-history-skip-empty-list
//! bug fix): visiting a rail category with no record focused must not push its own history entry, so
//! `⌘←`/`⌘→` never step through a bare, record-less list view. Opening or activating a record must
//! still push, matching the pattern in `dock.rs` (a probe component drives the hook, the test reads
//! the rendered marker).

use dioxus::prelude::*;
use genealogy_ui::{Category, Destination, RecordRef};
use genealogy_ui_dioxus::shell::nav_state::NavState;

fn record(human_id: &str, label: &str) -> RecordRef {
    RecordRef {
        category: Category::People,
        human_id: human_id.to_owned(),
        label: label.to_owned(),
    }
}

/// Renders a probe component to an HTML string.
fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// The marker block reflecting the history cursor state after the driving hook ran.
fn probe(nav: &NavState) -> Element {
    let back = if nav.can_back() { "YES" } else { "NO" };
    let forward = if nav.can_forward() { "YES" } else { "NO" };
    rsx! {
        div { "BACK:{back}" }
        div { "FORWARD:{forward}" }
    }
}

/// Two rail hops between category lists with no record ever opened or focused.
fn bare_category_hops() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.go_to(Destination::Category(Category::People));
        nav.go_to(Destination::Category(Category::Families));
    });
    probe(&nav)
}

#[test]
fn bare_category_hops_do_not_grow_the_history() {
    // Without the fix each hop pushes its own entry (Dashboard, People, Families), so `can_back`
    // would be true. The fix keeps the seeded Dashboard root as the only entry.
    let html = render(bare_category_hops);
    assert!(
        html.contains("BACK:NO"),
        "record-less list hops must not create back-history:\n{html}"
    );
    assert!(
        html.contains("FORWARD:NO"),
        "there is nothing to move forward to:\n{html}"
    );
}

/// A bare hop followed by opening a record — the record open must still push a history entry.
fn bare_hop_then_open_record() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.go_to(Destination::Category(Category::People)); // skipped: no record focused.
        nav.open_record(record("I0001", "Ada")); // always pushes: a record is now focused.
    });
    probe(&nav)
}

#[test]
fn opening_a_record_after_a_bare_hop_still_pushes_history() {
    let html = render(bare_hop_then_open_record);
    assert!(
        html.contains("BACK:YES"),
        "opening a record must still create a back-stop:\n{html}"
    );
}

/// Activating a second open record tab must still push a history entry.
fn open_two_then_activate_first() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        nav.activate_record(0); // back to Ada — always pushes: a record is always focused.
    });
    probe(&nav)
}

#[test]
fn activating_a_record_tab_still_pushes_history() {
    let html = render(open_two_then_activate_first);
    assert!(
        html.contains("BACK:YES"),
        "activating a record tab must still create a back-stop:\n{html}"
    );
}
