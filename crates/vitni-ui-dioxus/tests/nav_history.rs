//! SSR-probe assertions for back/forward navigation history on [`NavState`], following the pattern in
//! `dock.rs` (a probe component drives the hook, the test reads the rendered marker).
//!
//! Two rules are covered here. First (nav-history-skip-empty-list): visiting a rail category with no
//! record focused must not push its own history entry, so `⌘←`/`⌘→` never step through a bare,
//! record-less list view, while opening or activating a record still pushes. Second (#313): an open
//! **draft** tab is a stop like any other, and its stops track its lifetime — cancelling the draft drops
//! them, committing it rewrites them onto the record it stored, so the history never holds an entry that
//! resolves to no open tab.

use dioxus::prelude::*;
use vitni_ui::{Category, Destination, RecordRef};
use vitni_ui_dioxus::shell::nav_state::NavState;

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

/// How the active tab reads in the probe marker: a saved record by its `human_id`, a draft by its
/// [`DraftId`] (`DRAFT#1`), and `NONE` when no tab is active.
///
/// This is what makes a back/forward assertion say *where* it landed rather than only that it could
/// move: a history entry that resolves to no open tab clears the active record, which reads as `NONE`
/// here and is exactly the dead-entry symptom #313 is about.
fn active_marker(nav: &NavState) -> String {
    let Some(tab) = nav.active_tab() else {
        return "NONE".to_owned();
    };
    match (tab.human_id(), tab.draft_id()) {
        (Some(human_id), _) => human_id.to_owned(),
        (None, Some(draft)) => format!("DRAFT{draft}"),
        (None, None) => "NONE".to_owned(),
    }
}

/// The marker block reflecting the history cursor state, and what it resolved to, after the driving
/// hook ran.
fn probe(nav: &NavState) -> Element {
    let back = if nav.can_back() { "YES" } else { "NO" };
    let forward = if nav.can_forward() { "YES" } else { "NO" };
    let active = active_marker(nav);
    rsx! {
        div { "BACK:{back}" }
        div { "FORWARD:{forward}" }
        div { "ACTIVE:{active}" }
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

/// A record, then a draft opened over it, then one step back (#313).
fn record_then_draft_then_back() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_create(Category::People);
        nav.history_back();
    });
    probe(&nav)
}

#[test]
fn back_from_a_draft_returns_to_the_record_it_was_opened_over() {
    let html = render(record_then_draft_then_back);
    assert!(
        html.contains("ACTIVE:I0001"),
        "opening a draft is a back-stop, so back returns to where the operator came from:\n{html}"
    );
    assert!(
        html.contains("FORWARD:YES"),
        "the draft is still ahead in the history:\n{html}"
    );
}

/// The same, then one step forward again — the draft's own tab must come back.
fn record_then_draft_then_back_then_forward() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_create(Category::People);
        nav.history_back();
        nav.history_forward();
    });
    probe(&nav)
}

#[test]
fn forward_returns_to_the_open_draft_tab() {
    let html = render(record_then_draft_then_back_then_forward);
    assert!(
        html.contains("ACTIVE:DRAFT#1"),
        "forward must re-focus the draft's own tab, not step past it:\n{html}"
    );
    assert!(html.contains("FORWARD:NO"), "the draft is the newest entry:\n{html}");
}

/// Two records, a draft over them, the draft cancelled, then one step back.
fn two_records_then_a_cancelled_draft_then_back() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        let draft = nav.open_create(Category::People);
        nav.cancel_draft(draft);
        nav.history_back();
    });
    probe(&nav)
}

#[test]
fn a_cancelled_draft_leaves_no_stop_behind() {
    let html = render(two_records_then_a_cancelled_draft_then_back);
    assert!(
        html.contains("ACTIVE:I0001"),
        "cancelling drops the draft's entries, so back steps from Bob to Ada, not onto the dead draft:\n{html}"
    );
}

/// A record, a draft over it, the draft committed as a second record, then one step back.
fn record_then_a_committed_draft_then_back() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        let draft = nav.open_create(Category::People);
        nav.commit_draft(draft, record("I0002", "Bob"));
        nav.history_back();
    });
    probe(&nav)
}

#[test]
fn a_committed_draft_leaves_one_stop_on_the_record_it_stored() {
    let html = render(record_then_a_committed_draft_then_back);
    assert!(
        html.contains("ACTIVE:I0001"),
        "the commit re-keys the draft's entry, so exactly one back-step reaches Ada:\n{html}"
    );
    assert!(
        html.contains("FORWARD:YES"),
        "the re-keyed entry is still ahead:\n{html}"
    );
}

/// The same, then forward again — the re-keyed stop must resolve to the stored record's tab.
fn record_then_a_committed_draft_then_back_then_forward() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        let draft = nav.open_create(Category::People);
        nav.commit_draft(draft, record("I0002", "Bob"));
        nav.history_back();
        nav.history_forward();
    });
    probe(&nav)
}

#[test]
fn a_committed_drafts_stop_resolves_to_the_stored_records_tab() {
    let html = render(record_then_a_committed_draft_then_back_then_forward);
    assert!(
        html.contains("ACTIVE:I0002"),
        "the draft's stop now names the record the commit stored:\n{html}"
    );
    assert!(
        html.contains("FORWARD:NO"),
        "the commit left no second stop to step forward to:\n{html}"
    );
}

/// A record, a draft over it, the draft committed, then two steps back — which must reach the seeded
/// root, proving the commit re-keyed the draft's stop rather than adding one beside it.
fn record_then_a_committed_draft_then_two_backs() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        let draft = nav.open_create(Category::People);
        nav.commit_draft(draft, record("I0002", "Bob"));
        nav.history_back();
        nav.history_back();
    });
    probe(&nav)
}

#[test]
fn committing_a_draft_re_keys_its_stop_instead_of_adding_one() {
    let html = render(record_then_a_committed_draft_then_two_backs);
    assert!(
        html.contains("BACK:NO"),
        "the draft and the record it stored are one stop, so two backs reach the root:\n{html}"
    );
    assert!(
        html.contains("ACTIVE:NONE"),
        "the seeded root focuses no record:\n{html}"
    );
}
