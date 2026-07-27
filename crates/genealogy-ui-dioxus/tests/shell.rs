//! SSR assertions for the application shell (Phase 5 PR2, ADR 0008 §5): render the shell and its
//! overlays to HTML and assert the landmarks, roles, and localized labels the WCAG 2.2 AA gate
//! requires. Host-free — the shell's data screens read `AppCtx`, so the harness provides
//! `AppCtx::Failed` (the Person screen then renders nothing) instead of a real plugin host.
//!
//! Keyboard behaviour (`⌘K`/`?`/`Esc`/`g`-prefix/`⌘1…9`) and focus movement are *not* exercised
//! here — `onkeydown`/`set_focus` are inert under SSR. They are covered by the manual keyboard +
//! axe-core gate in the PR verification.

use std::collections::BTreeMap;
use std::rc::Rc;

use dioxus::prelude::*;
use genealogy_app::RecentItem;
use genealogy_ui::{Category, Destination, RecordRef, Tool};
use genealogy_ui_dioxus::app::AppCtx;
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::shell::help_overlay::HelpOverlay;
use genealogy_ui_dioxus::shell::nav_state::{NavState, Overlay};
use genealogy_ui_dioxus::shell::palette::CommandPalette;
use genealogy_ui_dioxus::shell::statusbar::ShellStatusbar;
use genealogy_ui_dioxus::shell::tabstrip::{NewRecordMenu, RecordTabstrip};
use genealogy_ui_dioxus::shell::topbar::Topbar;
use genealogy_ui_dioxus::shell::{ChromeCtx, Shell, ShortcutsCtx};
use unic_langid::LanguageIdentifier;

/// Opens a person record, then leaves the active destination at the given category.
fn person_record() -> RecordRef {
    RecordRef {
        category: Category::People,
        human_id: "I0001".to_owned(),
        label: "Ada Lovelace".to_owned(),
    }
}

/// A chrome localizer for a single explicit language (deterministic for tests).
fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

/// Renders a component to an HTML string.
fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// The full shell, localized to English; the Person screen renders nothing (no application state).
fn shell_en() -> Element {
    use_context_provider(|| AppCtx::Failed("test".to_owned()));
    use_context_provider(|| ChromeCtx(chrome("en")));
    rsx! {
        Shell {}
    }
}

/// The full shell, localized to Norwegian.
fn shell_no() -> Element {
    use_context_provider(|| AppCtx::Failed("test".to_owned()));
    use_context_provider(|| ChromeCtx(chrome("no")));
    rsx! {
        Shell {}
    }
}

/// The full shell with the help overlay forced open (via an injected `NavState`) so the background
/// `.app` should be inert and hidden from assistive tech (U3).
fn shell_with_overlay_open() -> Element {
    use_context_provider(|| AppCtx::Failed("test".to_owned()));
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(|| nav.overlay.set(Overlay::Help));
    rsx! {
        Shell {}
    }
}

/// The help overlay, forced open.
fn help_open() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(|| nav.overlay.set(Overlay::Help));
    rsx! {
        HelpOverlay {}
    }
}

/// The help overlay, forced open, with `quit` rebound to `⌘⇧Q` via a live [`ShortcutsCtx`]
/// (ADR 0030 §1): the overlay must render the *resolved* chord, not the default `⌘Q`.
fn help_open_with_rebound_quit() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    use_context_provider(|| {
        let bindings = BTreeMap::from([("quit".to_owned(), "mod+shift+q".to_owned())]);
        ShortcutsCtx(Signal::new(genealogy_app::ShortcutConfig { bindings }))
    });
    let mut nav = use_context_provider(NavState::new);
    use_hook(|| nav.overlay.set(Overlay::Help));
    rsx! {
        HelpOverlay {}
    }
}

/// The command palette, forced open.
fn palette_open() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(|| nav.overlay.set(Overlay::Palette));
    rsx! {
        CommandPalette {}
    }
}

/// The command palette, forced open and seeded with one recently-opened record (host-free, so the
/// record lists are absent and only the Commands + Recent groups render).
fn palette_with_recent() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(|| {
        nav.recent.set(vec![RecentItem::Record {
            kind: "family".to_owned(),
            human_id: "F0017".to_owned(),
            label: "Smith–Doe family".to_owned(),
        }]);
        nav.overlay.set(Overlay::Palette);
    });
    rsx! {
        CommandPalette {}
    }
}

/// The command palette, forced open, localized to Norwegian.
fn palette_open_no() -> Element {
    use_context_provider(|| ChromeCtx(chrome("no")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(|| nav.overlay.set(Overlay::Palette));
    rsx! {
        CommandPalette {}
    }
}

/// Both overlays with no overlay active — they must render nothing.
fn overlays_closed() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    use_context_provider(NavState::new);
    rsx! {
        CommandPalette {}
        HelpOverlay {}
    }
}

#[test]
fn shell_carries_landmarks_and_skip_link() {
    let html = render(shell_en);
    for needle in [
        r#"class="skip-link""#,
        r##"href="#main""##,
        r#"role="navigation""#,
        r#"aria-label="Primary""#,
        r#"role="banner""#,
        r#"role="search""#,
        r#"role="main""#,
        r#"id="main""#,
        r#"role="contentinfo""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in shell HTML:\n{html}");
    }
}

#[test]
fn an_open_overlay_inerts_and_hides_the_background_shell() {
    let open = render(shell_with_overlay_open);
    // `inert` only ever comes from the `.app` div; its presence means the background is inert.
    assert!(
        open.contains(r#"tabindex="-1" inert"#),
        "the background .app is inert while an overlay is open:\n{open}"
    );
    // The overlay itself still renders (it is a sibling of `.app`, never inerted).
    assert!(
        open.contains(r#"aria-modal="true""#),
        "the modal overlay renders alongside the inert background:\n{open}"
    );
    // With no overlay open the `.app` div carries neither inert nor aria-hidden.
    let closed = render(shell_en);
    assert!(
        !closed.contains("inert"),
        "the shell is interactive when no overlay is open:\n{closed}"
    );
}

#[test]
fn rail_lists_every_entity_and_tool() {
    let html = render(shell_en);
    for label in [
        "Dashboard",
        "People",
        "Families",
        "Events",
        "Places",
        "Sources",
        "Citations",
        "Repositories",
        "Media",
        "Notes",
        "Tags",
        "DNA tests",
        "DNA matches",
        "Pedigree",
        "Compare / merge",
        "Plugins",
        "Preferences",
        "Help",
    ] {
        assert!(
            html.contains(&format!(">{label}<")),
            "expected rail label {label:?}:\n{html}"
        );
    }
    assert!(html.contains("Entities"), "entities group heading:\n{html}");
    assert!(html.contains("Tools"), "tools group heading:\n{html}");
    // The default destination (Dashboard) is marked current, with roving tabindex on the rail.
    assert!(html.contains(r#"aria-current="page""#), "an active rail item:\n{html}");
    assert!(html.contains(r#"tabindex="0""#), "the roving tab stop:\n{html}");
    assert!(html.contains(r#"tabindex="-1""#), "the non-stop rail items:\n{html}");
}

#[test]
fn topbar_carries_search_and_controls() {
    let html = render(shell_en);
    assert!(
        html.contains(r#"aria-label="Breadcrumb""#),
        "breadcrumb landmark:\n{html}"
    );
    assert!(
        html.contains(r#"placeholder="Search people, places, sources…""#),
        "search placeholder:\n{html}"
    );
    assert!(
        html.contains(r#"class="sr-only""#),
        "visually-hidden search label:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Theme: System (click to change)""#),
        "theme-cycle accessible name (default System mode):\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Keyboard shortcuts""#),
        "help control accessible name:\n{html}"
    );
}

#[test]
fn shell_defaults_to_the_dark_theme_without_startup_prefs() {
    // No StartupPrefs context is provided here (as under SSR), so the shell falls back to System,
    // which resolves to the dark palette in the non-desktop build.
    let html = render(shell_en);
    assert!(
        html.contains(r#"data-theme="dark""#),
        "the shell root carries the resolved theme:\n{html}"
    );
}

/// The tabstrip + status bar on a record-browsing destination (People): unlike the dashboard, this
/// destination carries the open-records tab row.
fn record_chrome() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(|| nav.go_to(Destination::Category(Category::People)));
    rsx! {
        RecordTabstrip {}
        ShellStatusbar {}
    }
}

#[test]
fn tabstrip_and_statusbar_render() {
    let html = render(record_chrome);
    assert!(html.contains(r#"role="tablist""#), "tabstrip tablist:\n{html}");
    assert!(
        html.contains(r#"aria-label="Open records""#),
        "tabstrip accessible name:\n{html}"
    );
    // No record is open by default (host-free SSR cannot open one), so the strip shows only the
    // "open another" control — the per-tab `role="tab"`/`aria-selected` markup is exercised in the
    // keyboard/manual a11y gate, not here.
    assert!(
        html.contains(r#"class="rtab add""#),
        "the open-another-record control:\n{html}"
    );
    assert!(
        html.contains(r#"class="menu-anchor""#),
        "the '+' tab + its menu sit in a positioned anchor so the menu opens under the '+':\n{html}"
    );
    // The tabs live in an inner scroller; the anchor sits OUTSIDE it, because an `overflow` ancestor
    // would clip the absolutely-positioned menu (the prov-popover clipping hazard). The scroller
    // therefore renders before the anchor, never around it.
    let scroller = html.find(r#"class="tabs-scroll""#);
    let anchor = html.find(r#"class="menu-anchor""#);
    assert!(
        scroller.is_some_and(|scroller| anchor.is_some_and(|anchor| scroller < anchor)),
        "the tab scroller renders, with the menu anchor following as a sibling:\n{html}"
    );
    assert!(
        html.contains(r#"class="active-record""#),
        "status bar active record:\n{html}"
    );
    assert!(
        html.contains(r#"aria-live="polite""#),
        "status bar live region:\n{html}"
    );
}

/// Navigates to a tool destination (Pedigree) — never opens a record.
fn tool_visit() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(|| nav.go_to(Destination::Tool(Tool::Pedigree)));
    rsx! { span { "{nav.recent.read().len()}" } }
}

#[test]
fn navigating_to_a_tool_does_not_record_a_recent_entry() {
    // The dashboard "Jump back in" list remembers records only — visiting a tool must leave the
    // persisted recent list untouched (only `NavState::open_record` pushes to it).
    let html = render(tool_visit);
    assert!(
        html.contains(">0<"),
        "a tool visit must not push a recent entry:\n{html}"
    );
}

/// The record tabstrip rendered in isolation with no records open: the control row (back/forward,
/// the new-record "+") still renders, but there are no per-record tabs. The shell now mounts the
/// tabstrip only on entity categories, not the Dashboard (`entity_category`); this exercises the
/// component's empty state directly, independent of that shell wiring.
fn dashboard_tabstrip() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    use_context_provider(NavState::new);
    rsx! {
        RecordTabstrip {}
    }
}

/// The top bar with a person record open while the Dashboard is the active destination.
fn topbar_on_dashboard_with_open_record() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.open_record(person_record())); // active stays Dashboard (default)
    rsx! {
        Topbar {}
    }
}

/// The top bar with the same record open while People (its own category) is active.
fn topbar_on_people_with_open_record() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(person_record());
        nav.go_to(Destination::Category(Category::People));
    });
    rsx! {
        Topbar {}
    }
}

#[test]
fn breadcrumb_trails_the_record_only_on_its_own_screen() {
    // On the dashboard the open record is not part of the breadcrumb.
    let dashboard = render(topbar_on_dashboard_with_open_record);
    assert!(
        !dashboard.contains("Ada Lovelace"),
        "the dashboard breadcrumb omits the open record:\n{dashboard}"
    );
    // On People (the record's category) it trails the record.
    let people = render(topbar_on_people_with_open_record);
    assert!(
        people.contains("Ada Lovelace"),
        "the People breadcrumb shows the active record:\n{people}"
    );
}

#[test]
fn the_dashboard_tab_row_has_no_record_tabs() {
    // Removing the early return (WP2-7) means the control row itself always renders, so back/forward
    // stays reachable — but with no records open, there are no per-record `role="tab"` buttons.
    let html = render(dashboard_tabstrip);
    assert!(
        html.contains(r#"role="tablist""#),
        "the control row still renders on the dashboard:\n{html}"
    );
    assert!(
        !html.contains(r#"role="tab""#),
        "no open record tabs on the dashboard:\n{html}"
    );
    assert!(
        html.contains(r#"class="rtab add""#),
        "the new-record control still renders:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Back""#),
        "the back-history control:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Forward""#),
        "the forward-history control:\n{html}"
    );
}

#[test]
fn help_overlay_renders_the_shortcut_map() {
    let html = render(help_open);
    assert!(html.contains(r#"class="help-sheet""#), "help sheet:\n{html}");
    assert!(html.contains(r#"role="dialog""#), "help dialog role:\n{html}");
    assert!(html.contains(r#"aria-modal="true""#), "help is modal:\n{html}");
    assert!(
        html.contains(r#"aria-label="Keyboard shortcuts""#),
        "help title:\n{html}"
    );
    for heading in ["Global", "Go to", "Within a screen"] {
        assert!(html.contains(heading), "help column {heading:?}:\n{html}");
    }
    assert!(
        html.contains("Command palette"),
        "a global shortcut description:\n{html}"
    );
    assert!(html.contains("Quit"), "the quit shortcut row:\n{html}");
    assert!(
        html.contains("Close tab"),
        "the close-current-tab shortcut row:\n{html}"
    );
    // The "Go to" column lists bare category names paired with their `g`-prefix second key.
    assert!(html.contains(">People<"), "a g-prefix navigation row:\n{html}");
    assert!(html.contains("<kbd>g</kbd><kbd>p</kbd>"), "the g p chord:\n{html}");
}

#[test]
fn help_overlay_reflects_a_rebound_global_chord() {
    let html = render(help_open_with_rebound_quit);
    assert!(
        html.contains("<kbd>Ctrl⇧</kbd><kbd>Q</kbd>"),
        "the rebound chord (⌘⇧Q) renders instead of the default ⌘Q:\n{html}"
    );
}

#[test]
fn palette_renders_as_a_modal_dialog() {
    let html = render(palette_open);
    assert!(html.contains(r#"class="overlay""#), "palette backdrop:\n{html}");
    assert!(html.contains(r#"class="palette""#), "palette surface:\n{html}");
    assert!(html.contains(r#"role="dialog""#), "palette dialog role:\n{html}");
    assert!(html.contains(r#"aria-modal="true""#), "palette is modal:\n{html}");
    assert!(
        html.contains(r#"placeholder="Type a command or search…""#),
        "palette input placeholder:\n{html}"
    );
    // The input is an ARIA combobox wired to the results listbox with an active option.
    assert!(html.contains(r#"role="combobox""#), "combobox role:\n{html}");
    assert!(
        html.contains(r#"aria-autocomplete="list""#),
        "combobox autocomplete:\n{html}"
    );
    assert!(
        html.contains(r#"aria-controls="palette-listbox""#),
        "combobox controls the listbox:\n{html}"
    );
    assert!(
        html.contains(r#"aria-activedescendant="palette-opt-0""#),
        "the first option is the active descendant:\n{html}"
    );
    assert!(html.contains(r#"id="palette-listbox""#), "the results listbox:\n{html}");
    assert!(html.contains(r#"role="listbox""#), "listbox role:\n{html}");
    // Commands render even host-free (no workspace); the Commands group heading is present and the
    // first option is selected.
    assert!(html.contains("Commands"), "the Commands group heading:\n{html}");
    assert!(html.contains(r#"role="option""#), "at least one option:\n{html}");
    assert!(html.contains(r#"id="palette-opt-0""#), "the first option id:\n{html}");
    assert!(
        html.contains(r#"aria-selected="true""#),
        "the first option is selected:\n{html}"
    );
    // The footer hint row renders the navigate/open cues.
    assert!(html.contains("navigate"), "the navigate footer hint:\n{html}");
    assert!(html.contains("open"), "the open footer hint:\n{html}");
}

#[test]
fn palette_lists_recent_records_when_seeded() {
    let html = render(palette_with_recent);
    assert!(html.contains("Recent"), "the Recent group heading:\n{html}");
    assert!(html.contains("Smith–Doe family"), "the seeded recent record:\n{html}");
}

#[test]
fn palette_localizes_commands_to_norwegian() {
    let html = render(palette_open_no);
    assert!(
        html.contains("Kommandoer"),
        "the Commands group heading in Norwegian:\n{html}"
    );
    assert!(
        html.contains(r#"placeholder="Skriv en kommando eller søk…""#),
        "the Norwegian placeholder:\n{html}"
    );
}

#[test]
fn closed_overlays_render_nothing() {
    let html = render(overlays_closed);
    assert!(
        !html.contains(r#"class="palette""#),
        "closed palette renders nothing:\n{html}"
    );
    assert!(
        !html.contains(r#"class="help-sheet""#),
        "closed help renders nothing:\n{html}"
    );
}

#[test]
fn shell_localizes_to_norwegian() {
    let html = render(shell_no);
    for label in ["Personer", "Familier", "Kilder", "Sitater", "Arkiv", "Innstillinger"] {
        assert!(
            html.contains(&format!(">{label}<")),
            "expected Norwegian label {label:?}:\n{html}"
        );
    }
    assert!(
        html.contains(r#"placeholder="Søk i personer, steder, kilder…""#),
        "Norwegian search placeholder:\n{html}"
    );
}

/// The tabstrip's new-record menu, forced open — one `role="menuitem"` per creatable category.
fn new_record_menu_open() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    use_context_provider(NavState::new);
    let open = use_signal(|| true);
    rsx! {
        NewRecordMenu { open }
    }
}

#[test]
fn new_record_menu_lists_every_creatable_category() {
    let html = render(new_record_menu_open);
    assert!(html.contains(r#"role="menu""#), "menu role:\n{html}");
    assert_eq!(
        html.matches(r#"role="menuitem""#).count(),
        12,
        "one menuitem per creatable category:\n{html}"
    );
    for label in ["People", "Families", "DNA tests", "DNA matches"] {
        assert!(
            html.contains(&format!(">{label}<")),
            "expected category label {label:?}:\n{html}"
        );
    }
}

#[test]
fn new_record_menu_scrim_is_a_click_away_sibling_not_the_menu_parent() {
    let html = render(new_record_menu_open);
    assert!(
        html.contains(r#"class="menu-scrim""#),
        "the click-away scrim renders:\n{html}"
    );
    // The scrim is an empty `<button>` sibling — the menu is anchored under the "+", not nested inside
    // a viewport-covering scrim. Prove the menu opens after the scrim button closes.
    let scrim_close = html
        .find("menu-scrim")
        .and_then(|start| html[start..].find("</button>").map(|offset| start + offset))
        .unwrap_or(usize::MAX);
    let menu = html.find("new-record-menu").unwrap_or(0);
    assert!(
        scrim_close != usize::MAX && menu > scrim_close,
        "the menu is a sibling after the scrim, never its child:\n{html}"
    );
}
