//! SSR-probe assertions for per-pane scoping in a docked split (issue #279): two mounted detail
//! panes must not collide on ARIA ids and must not answer for each other's undo request.
//!
//! `PaneRole` is provided only by `DockedRecordDetail` (`screens/record_detail.rs`), so a single
//! (undocked) pane sees no `PaneRole` in context at all — `single_pane` below is the lock that keeps
//! `tests/components.rs:138` and `tests/master_detail.rs:139` passing unchanged. `DockedPane` mimics
//! `DockedRecordDetail` by providing `PaneRole::Docked` as its first hook, ahead of its child.

use dioxus::prelude::*;
use vitni_ui::Category;
use vitni_ui_dioxus::components::TabItem;
use vitni_ui_dioxus::master_detail::DetailContainer;
use vitni_ui_dioxus::screens::use_record_undo;
use vitni_ui_dioxus::shell::nav_state::{EditKey, NavState, PaneRole};

/// Renders a probe component to an HTML string.
fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// Renders a probe and settles it (`use_effect` bodies run only after a render pass) — see
/// `tests/edit_stash.rs`'s helper of the same name.
fn render_settled(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    for _ in 0..8 {
        vdom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
    dioxus_ssr::render(&vdom)
}

/// Two tabs, standing in for a real detail pane's related-item strip.
fn tabs() -> Vec<TabItem> {
    vec![
        TabItem {
            id: "overview".to_owned(),
            label: "Overview".to_owned(),
            count: None,
        },
        TabItem {
            id: "citations".to_owned(),
            label: "Citations".to_owned(),
            count: Some(2),
        },
    ]
}

/// A stand-in record pane: a `DetailContainer` over [`tabs`], the same shape every real detail pane
/// renders.
#[component]
fn Pane(title: String) -> Element {
    let active = use_signal(|| 0_usize);
    rsx! {
        DetailContainer {
            title,
            actions: rsx! {},
            extras: rsx! {},
            tabs: tabs(),
            active,
            div { "body" }
        }
    }
}

/// A docked pane: provides `PaneRole::Docked` ahead of its child, exactly as
/// `screens::DockedRecordDetail` does ahead of the record it renders.
#[component]
fn DockedPane(title: String) -> Element {
    use_context_provider(|| PaneRole::Docked);
    rsx! {
        Pane { title }
    }
}

fn single_pane() -> Element {
    rsx! {
        Pane { title: "Ada" }
    }
}

fn two_panes() -> Element {
    rsx! {
        Pane { title: "Ada" }
        DockedPane { title: "Bob" }
    }
}

#[test]
fn a_single_pane_keeps_unprefixed_ids() {
    // The lock: no `PaneRole` in context (the undocked case) must render exactly today's markup.
    let html = render(single_pane);
    assert!(html.contains(r#"id="tab-overview""#), "unprefixed tab id:\n{html}");
    assert!(
        html.contains(r#"aria-controls="panel-overview""#),
        "unprefixed aria-controls:\n{html}"
    );
    assert!(html.contains(r#"id="panel-overview""#), "unprefixed panel id:\n{html}");
}

#[test]
fn two_mounted_panes_emit_distinct_tab_and_panel_ids() {
    let html = render(two_panes);
    assert!(
        html.contains(r#"id="tab-docked-overview""#),
        "the docked pane's tab id is scoped:\n{html}"
    );
    assert!(
        html.contains(r#"aria-controls="panel-docked-overview""#),
        "the docked pane's aria-controls is scoped:\n{html}"
    );
    assert!(
        html.contains(r#"id="panel-docked-overview""#),
        "the docked pane's panel id is scoped:\n{html}"
    );
    assert_eq!(
        html.matches(r#"id="tab-overview""#).count(),
        1,
        "the unprefixed id occurs exactly once even with two panes mounted:\n{html}"
    );
}

#[test]
fn the_docked_panes_title_is_an_h2_under_the_active_panes_h1() {
    let html = render(two_panes);
    assert_eq!(
        html.matches("<h1").count(),
        1,
        "the active pane keeps the screen's single <h1>:\n{html}"
    );
    assert!(
        html.contains(r#"<h2 class="detail-title""#),
        "the docked pane's own heading is a subordinate <h2>:\n{html}"
    );
}

/// The `⌘Z` targets collected by two probes sharing this context.
#[derive(Clone, Copy)]
struct Fired(Signal<Vec<String>>);

/// A history entry that is always undoable, so an addressed pane always has something to fire.
fn undoable_entry() -> Vec<vitni_ui::HistoryEntryVm> {
    vec![vitni_ui::HistoryEntryVm {
        when: "2026-01-01 00:00".to_owned(),
        what: "Name asserted".to_owned(),
        who: "test".to_owned(),
        why: None,
        assertion_id: "a1".to_owned(),
        can_undo: true,
    }]
}

/// A stand-in detail pane exercising only `use_record_undo`, recording into the shared [`Fired`]
/// context when the shell's request names it.
#[component]
fn UndoPane(human_id: String) -> Element {
    let nav = use_context::<NavState>();
    let mut fired = use_context::<Fired>();
    let busy = use_memo(|| false);
    let history = use_memo(undoable_entry);
    let recorded = human_id.clone();
    let on_undo = use_callback(move |_assertion_id: String| {
        fired.0.write().push(recorded.clone());
    });
    use_record_undo(
        nav,
        Category::People,
        &human_id,
        busy,
        history,
        "nothing".to_owned(),
        on_undo,
    );
    rsx! { div { "PANE:{human_id}" } }
}

fn undo_addressed_to_one_pane() -> Element {
    let mut nav = use_context_provider(NavState::new);
    let fired = use_context_provider(|| Fired(Signal::new(Vec::new())));
    use_hook(move || {
        nav.pending_undo.set(Some(EditKey::saved(Category::People, "I0002")));
    });
    let fired_list = fired.0.read().join(",");
    let pending = if nav.pending_undo.read().is_some() {
        "SOME"
    } else {
        "NONE"
    };
    rsx! {
        UndoPane { human_id: "I0001" }
        UndoPane { human_id: "I0002" }
        div { "FIRED:{fired_list}" }
        div { "PENDING:{pending}" }
    }
}

#[test]
fn an_undo_aimed_at_one_record_is_ignored_by_the_other_pane() {
    let html = render_settled(undo_addressed_to_one_pane);
    assert!(
        html.contains("FIRED:I0002"),
        "only the addressed pane's on_undo fires:\n{html}"
    );
    assert!(
        !html.contains("FIRED:I0001") && !html.contains("FIRED:I0001,I0002") && !html.contains("FIRED:I0002,I0001"),
        "the unaddressed pane never fires:\n{html}"
    );
}

#[test]
fn the_serving_pane_clears_the_request() {
    let html = render_settled(undo_addressed_to_one_pane);
    assert!(
        html.contains("PENDING:NONE"),
        "the pane that handled the request clears it:\n{html}"
    );
}

fn draft_active_never_arms_undo() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::Tags);
        nav.request_undo();
    });
    let pending = if nav.pending_undo.read().is_some() {
        "SOME"
    } else {
        "NONE"
    };
    rsx! { div { "PENDING:{pending}" } }
}

#[test]
fn requesting_undo_on_an_active_draft_arms_nothing() {
    // A draft has no undo hook (`use_record_undo` is never called for one), so arming a request for
    // it would stick — `request_undo` must no-op instead.
    let html = render(draft_active_never_arms_undo);
    assert!(
        html.contains("PENDING:NONE"),
        "a draft active tab never arms an undo request:\n{html}"
    );
}
