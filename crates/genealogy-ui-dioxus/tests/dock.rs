//! SSR-probe assertions for the record-tab docking lifecycle on [`NavState`] (Phase 5 PR36).
//!
//! `onkeydown`/drag events are inert under SSR, so each probe component provides a `NavState`,
//! drives its dock methods in `use_hook`, and renders a small marker block that the test inspects:
//! `REF:` is the resolved [`NavState::docked_record_ref`] label (or `NONE`), `RAW:` is whether the
//! raw `docked_record` key is set, `DRAG:` whether a tab drag is live. The manual desktop check
//! covers the actual drag gesture and `⌘⇧1…9` keys.

use std::rc::Rc;

use dioxus::prelude::*;
use genealogy_ui::{Category, RecordRef};
use genealogy_ui_dioxus::app::AppCtx;
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::master_detail::MasterDetail;
use genealogy_ui_dioxus::shell::ChromeCtx;
use genealogy_ui_dioxus::shell::nav_state::NavState;
use genealogy_ui_dioxus::shell::tabstrip::RecordTabstrip;
use unic_langid::LanguageIdentifier;

/// A chrome localizer for a single explicit language (deterministic for tests).
fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

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

/// The marker block reflecting the dock state after the driving hook ran.
fn probe(nav: &NavState) -> Element {
    let reference = nav
        .docked_record_ref()
        .map_or_else(|| "NONE".to_owned(), |record| record.label);
    let raw = if nav.docked_record.read().is_some() {
        "SOME"
    } else {
        "NONE"
    };
    let drag = if nav.dragging_tab.read().is_some() {
        "SOME"
    } else {
        "NONE"
    };
    rsx! {
        div { "REF:{reference}" }
        div { "RAW:{raw}" }
        div { "DRAG:{drag}" }
    }
}

/// Opens Ada (I0001) then Bob (I0002); Bob is the active record (last opened).
fn open_two(nav: &mut NavState) {
    nav.open_record(record("I0001", "Ada"));
    nav.open_record(record("I0002", "Bob"));
}

fn dock_inactive() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        open_two(&mut nav);
        nav.dock_record(Category::People, "I0001");
    });
    probe(&nav)
}

fn dock_active_is_noop() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        open_two(&mut nav);
        nav.dock_record(Category::People, "I0002");
    });
    probe(&nav)
}

fn dock_toggles_off() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        open_two(&mut nav);
        nav.dock_record(Category::People, "I0001");
        nav.dock_record(Category::People, "I0001");
    });
    probe(&nav)
}

fn dock_unknown_key_is_noop() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        open_two(&mut nav);
        nav.dock_record(Category::People, "I9999");
    });
    probe(&nav)
}

fn dock_then_activate_docked() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        open_two(&mut nav);
        nav.dock_record(Category::People, "I0001");
        nav.activate_record(0); // Ada (the docked tab) becomes active.
    });
    probe(&nav)
}

fn dock_then_reactivate_other() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        open_two(&mut nav);
        nav.dock_record(Category::People, "I0001");
        nav.activate_record(0); // Ada active — split collapses.
        nav.activate_record(1); // Bob active again — split returns.
    });
    probe(&nav)
}

fn close_docked_clears() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        open_two(&mut nav);
        nav.dock_record(Category::People, "I0001");
        nav.close_record(0); // close the docked tab.
    });
    probe(&nav)
}

fn close_unrelated_keeps() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.open_record(record("I0002", "Bob"));
        nav.open_record(record("I0003", "Cara")); // Cara active.
        nav.dock_record(Category::People, "I0001");
        nav.close_record(1); // close Bob — Ada's index shifts, dock must survive by key.
    });
    probe(&nav)
}

fn rename_rekeys_dock() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        open_two(&mut nav);
        nav.dock_record(Category::People, "I0001");
        nav.rename_record(Category::People, "I0001", "I0009".to_owned());
    });
    probe(&nav)
}

fn drag_begin_then_complete() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        open_two(&mut nav);
        nav.begin_tab_drag(Category::People, "I0001");
        nav.complete_tab_drag();
    });
    probe(&nav)
}

fn complete_without_drag_is_noop() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        open_two(&mut nav);
        nav.complete_tab_drag();
    });
    probe(&nav)
}

#[test]
fn docking_an_inactive_tab_sets_the_dock() {
    let html = render(dock_inactive);
    assert!(html.contains("REF:Ada"), "docked ref resolves to Ada:\n{html}");
    assert!(html.contains("RAW:SOME"), "raw dock key is set:\n{html}");
}

#[test]
fn docking_the_active_record_is_a_noop() {
    let html = render(dock_active_is_noop);
    assert!(html.contains("REF:NONE"), "no split for the active record:\n{html}");
    assert!(html.contains("RAW:NONE"), "raw dock key stays unset:\n{html}");
}

#[test]
fn docking_the_same_tab_twice_toggles_off() {
    let html = render(dock_toggles_off);
    assert!(html.contains("REF:NONE"), "second dock toggles off:\n{html}");
    assert!(html.contains("RAW:NONE"), "raw dock key cleared:\n{html}");
}

#[test]
fn docking_an_unknown_key_is_a_noop() {
    let html = render(dock_unknown_key_is_noop);
    assert!(html.contains("RAW:NONE"), "unknown key never docks:\n{html}");
}

#[test]
fn activating_the_docked_tab_collapses_the_split_but_keeps_state() {
    let html = render(dock_then_activate_docked);
    assert!(
        html.contains("REF:NONE"),
        "split collapses while docked tab is active:\n{html}"
    );
    assert!(html.contains("RAW:SOME"), "raw dock state survives:\n{html}");
}

#[test]
fn reactivating_another_record_returns_the_split() {
    let html = render(dock_then_reactivate_other);
    assert!(
        html.contains("REF:Ada"),
        "split returns when the docked tab is not active:\n{html}"
    );
}

#[test]
fn closing_the_docked_tab_clears_the_dock() {
    let html = render(close_docked_clears);
    assert!(
        html.contains("RAW:NONE"),
        "closing the docked tab clears the dock:\n{html}"
    );
}

#[test]
fn closing_an_unrelated_tab_keeps_the_dock() {
    let html = render(close_unrelated_keeps);
    assert!(
        html.contains("REF:Ada"),
        "dock survives an unrelated close (key-based):\n{html}"
    );
}

#[test]
fn renaming_the_docked_record_rekeys_the_dock() {
    let html = render(rename_rekeys_dock);
    assert!(
        html.contains("REF:Ada"),
        "dock follows the record to its new id:\n{html}"
    );
    assert!(html.contains("RAW:SOME"), "dock key survives the rename:\n{html}");
}

#[test]
fn a_full_drag_sets_the_dock_and_clears_the_drag() {
    let html = render(drag_begin_then_complete);
    assert!(
        html.contains("REF:Ada"),
        "completing the drag docks the dragged tab:\n{html}"
    );
    assert!(html.contains("DRAG:NONE"), "the drag state clears:\n{html}");
}

#[test]
fn completing_without_a_drag_is_a_noop() {
    let html = render(complete_without_drag_is_noop);
    assert!(html.contains("RAW:NONE"), "no drag means no dock:\n{html}");
}

/// The record tabstrip with Ada docked while Bob is active (host-free chrome).
fn tabstrip_with_dock() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        open_two(&mut nav);
        nav.dock_record(Category::People, "I0001");
    });
    rsx! {
        RecordTabstrip {}
    }
}

#[test]
fn tabstrip_tabs_are_draggable() {
    let html = render(tabstrip_with_dock);
    assert!(
        html.contains(r#"draggable="true""#),
        "each record tab is a drag source:\n{html}"
    );
}

#[test]
fn each_tab_close_control_is_keyboard_operable_and_row_scoped() {
    // U4: the close control is focusable (tabindex=0), operable, and names the record it closes
    // (not the generic "Close record"), so it announces per tab.
    let html = render(tabstrip_with_dock);
    assert!(
        html.contains(r#"class="close" role="button" tabindex="0" aria-label="Close Ada""#),
        "Ada's close control is a focusable, row-scoped button:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Close Bob""#),
        "Bob's close control names its own record:\n{html}"
    );
    assert!(
        !html.contains(r#"aria-label="Close record""#),
        "the generic close label is replaced by the record-scoped one:\n{html}"
    );
}

#[test]
fn the_docked_tab_carries_the_docked_class() {
    let html = render(tabstrip_with_dock);
    assert!(
        html.contains(r#"class="rtab docked""#),
        "the docked (inactive) tab is marked docked:\n{html}"
    );
    assert!(
        html.contains(r#"class="rtab active""#),
        "the active (undocked) tab keeps its class:\n{html}"
    );
}

/// A master-detail screen with Ada docked beside the active Bob. `AppCtx::Failed` makes the docked
/// pane's inner detail render nothing (host-free), but the docked-head label and undock control
/// still render, and the layout becomes `split-2`.
fn master_detail_with_dock() -> Element {
    use_context_provider(|| AppCtx::Failed("test".to_owned()));
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        open_two(&mut nav);
        nav.dock_record(Category::People, "I0001");
    });
    rsx! {
        MasterDetail { detail: rsx! { p { "primary" } } }
    }
}

/// A master-detail screen rendered with no `NavState` in context (the bare framework test path):
/// no split, no docked pane.
fn master_detail_no_nav() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    rsx! {
        MasterDetail { detail: rsx! { p { "primary" } } }
    }
}

#[test]
fn a_docked_record_splits_the_detail_pane() {
    let html = render(master_detail_with_dock);
    assert!(
        html.contains(r#"class="master-detail split-2""#),
        "the layout becomes a three-column split:\n{html}"
    );
    assert!(
        html.contains(r#"class="detail docked""#),
        "a second, docked detail pane renders:\n{html}"
    );
    assert!(html.contains("Ada"), "the docked pane names the docked record:\n{html}");
    assert!(
        html.contains(r#"aria-label="Undock record""#),
        "the undock control carries the localized label:\n{html}"
    );
}

#[test]
fn no_navstate_renders_a_single_pane() {
    let html = render(master_detail_no_nav);
    assert!(
        html.contains(r#"class="master-detail""#),
        "the single-pane layout renders:\n{html}"
    );
    assert!(!html.contains("split-2"), "no split without a dock:\n{html}");
    assert!(
        !html.contains("detail docked"),
        "no docked pane without a dock:\n{html}"
    );
}
