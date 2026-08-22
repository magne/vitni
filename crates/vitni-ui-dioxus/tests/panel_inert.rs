//! SSR assertions for issue #312: while a `SidePanel` is open, everything *behind* it is `inert` and
//! hidden from assistive tech, and the panel itself is not.
//!
//! A panel renders inside `.app`, so the overlay trick of inerting `.app` cannot be used — it would
//! inert the panel with it. Instead every chrome region behind a panel inerts its own root, and each
//! panel registers its scope in `NavState::open_panels` while it is open. These tests cover both
//! halves: the pane the panel sits beside (its `DetailContainer`), and the shell regions around it.
//!
//! The panel must not be a *descendant* of anything inerted, so the ordering assertions here walk the
//! rendered `<div` / `</div>` nesting to find where each inerted element actually closes, rather than
//! merely checking that the panel markup comes later in the string.

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_ui::{Category, Destination};
use vitni_ui_dioxus::app::AppCtx;
use vitni_ui_dioxus::components::{SidePanel, TabItem};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::master_detail::DetailContainer;
use vitni_ui_dioxus::shell::nav_state::NavState;
use vitni_ui_dioxus::shell::{ChromeCtx, Shell};

/// The scope id a shell-level test registers as "a panel is open": the shell renders no panel of its
/// own (its record panes need application state), so the registry is seeded directly with the same
/// call `SidePanel` makes.
const PANEL_SCOPE: ScopeId = ScopeId(1);

/// Renders a view to HTML with a second render pass in between: `SidePanel` registers itself
/// *during* render (`dioxus-ssr` never runs effects), and that write only reaches the ancestors
/// already rendered in the first pass on the next one.
fn render_twice(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    let _ = vdom.render_immediate_to_vec();
    dioxus_ssr::render(&vdom)
}

/// Renders a view to HTML in one pass, like the other shell tests do. The shell fixtures below seed
/// the registry in a `use_hook` *above* the shell, so it is already populated on the first pass —
/// and the `desktop` build's window managers, which panic host-free, must be rendered only once.
fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// A chrome localizer for a single explicit language (deterministic for tests).
fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

/// A record pane's shape: a `DetailContainer` with its side panel as a *sibling*, which is how every
/// `*DetailPane` is built (e.g. `screens/note.rs`).
fn pane(open: bool) -> Element {
    use_context_provider(NavState::new);
    let active = use_signal(|| 0_usize);
    let onclose = use_callback(|(): ()| {});
    rsx! {
        DetailContainer {
            title: "Ada Lovelace".to_owned(),
            id_label: "I0001".to_owned(),
            extras: rsx! {},
            actions: rsx! { button { "Edit" } },
            tabs: vec![TabItem { id: "overview".to_owned(), label: "Overview".to_owned(), count: None }],
            active,
            p { "overview body" }
        }
        SidePanel {
            title: "Edit name".to_owned(),
            open,
            close_label: "Close".to_owned(),
            onclose,
            footer: rsx! { button { "Save" } },
            div { "panel body" }
        }
    }
}

/// The pane with its side panel open.
fn pane_with_panel_open() -> Element {
    pane(true)
}

/// The same pane with the panel closed — the component stays mounted, it just renders nothing.
fn pane_with_panel_closed() -> Element {
    pane(false)
}

/// The full shell on an entity category (so the Explorer and the record tabstrip mount too), with a
/// panel registered as open.
fn shell_with_panel_open() -> Element {
    use_context_provider(|| AppCtx::Failed("test".to_owned()));
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.go_to(Destination::Category(Category::People));
        nav.open_panel(PANEL_SCOPE);
    });
    rsx! {
        Shell {}
    }
}

/// The same shell with no panel registered.
fn shell_without_panel() -> Element {
    use_context_provider(|| AppCtx::Failed("test".to_owned()));
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.go_to(Destination::Category(Category::People)));
    rsx! {
        Shell {}
    }
}

/// The byte index of `needle` in `html`, or `usize::MAX` when it is absent — so a missing marker fails
/// the ordering assertion that reads it rather than passing by accident.
fn at(html: &str, needle: &str) -> usize {
    html.find(needle).unwrap_or(usize::MAX)
}

/// The byte index just past the `</div>` closing the `<div …>` that starts at `start`, tracking
/// `<div` / `</div>` nesting. `html.len()` when it never closes.
fn div_end(html: &str, start: usize) -> usize {
    let mut depth = 0_usize;
    let mut index = start;
    while index < html.len() {
        let rest = &html[index..];
        let open = rest.find("<div").map(|offset| index + offset);
        let close = rest.find("</div>").map(|offset| index + offset);
        match (open, close) {
            (Some(open), Some(close)) if open < close => index = open + "<div".len(),
            (_, Some(close)) => {
                index = close + "</div>".len();
                depth = depth.saturating_sub(1);
                if depth == 0 {
                    return index;
                }
                continue;
            }
            (Some(open), None) => index = open + "<div".len(),
            (None, None) => return html.len(),
        }
        depth += 1;
    }
    html.len()
}

/// The opening tag of the first element whose attributes contain `marker` (e.g. `class="rail"`), so an
/// assertion can be made against that one element's attributes whatever order they render in.
fn open_tag<'a>(html: &'a str, marker: &str) -> &'a str {
    let Some(marker_at) = html.find(marker) else {
        return "";
    };
    let start = html[..marker_at].rfind('<').unwrap_or(0);
    let end = html[start..].find('>').map_or(html.len(), |offset| start + offset + 1);
    &html[start..end]
}

/// Asserts that the element `marker` names is inert *and* hidden from assistive tech.
fn assert_inert(html: &str, marker: &str) {
    let tag = open_tag(html, marker);
    assert!(
        tag.contains(r#"inert="true""#) && tag.contains(r#"aria-hidden="true""#),
        "expected {marker} to be inert and aria-hidden, got {tag:?}:\n{html}"
    );
}

#[test]
fn an_open_panel_inerts_the_pane_it_sits_beside() {
    let html = render_twice(pane_with_panel_open);
    // Every root `DetailContainer` renders — the header, the tab strip, and the tab body (its `Tabs`
    // is a fragment of the last two) — is behind the panel and inert.
    for marker in [r#"class="detail-head""#, r#"class="tabs""#, r#"class="tab-body""#] {
        assert_inert(&html, marker);
    }
    // And the panel is not inert itself: it carries no such attribute at all.
    let panel = open_tag(&html, r#"class="sidepanel""#);
    assert!(
        !panel.contains("inert") && !panel.contains("aria-hidden"),
        "the panel itself is interactive, got {panel:?}:\n{html}"
    );
}

#[test]
fn the_open_panel_is_not_a_descendant_of_the_inerted_subtree() {
    let html = render_twice(pane_with_panel_open);
    let scrim = at(&html, r#"class="sidepanel-scrim""#);
    let panel = at(&html, r#"class="sidepanel""#);
    // Every inerted element in the pane, whichever they are, must have closed again before the panel's
    // own markup begins — `inert` is inherited and no descendant can undo it.
    let mut inerted = 0_usize;
    let mut searched = 0_usize;
    while let Some(offset) = html[searched..].find(r#"inert="true""#) {
        let attribute = searched + offset;
        let start = html[..attribute].rfind('<').unwrap_or(0);
        let closed = div_end(&html, start);
        assert!(
            closed <= scrim && closed <= panel,
            "the inerted element at {start} closes at {closed}, after the panel's scrim ({scrim}) or \
             body ({panel}) — the panel is inside the inerted subtree:\n{html}"
        );
        inerted += 1;
        searched = attribute + 1;
    }
    assert!(inerted > 0, "the pane behind the open panel is inerted at all:\n{html}");
}

#[test]
fn a_closed_panel_leaves_the_pane_interactive() {
    let html = render_twice(pane_with_panel_closed);
    assert!(
        html.contains(r#"class="detail-head""#),
        "the pane still renders with the panel closed:\n{html}"
    );
    assert!(
        !html.contains("inert"),
        "nothing is inert while no panel is open:\n{html}"
    );
}

#[test]
fn an_open_panel_inerts_every_shell_region_behind_it() {
    let html = render(shell_with_panel_open);
    for marker in [
        r#"class="skip-link""#,
        r#"class="rail""#,
        r#"class="list""#,
        r#"class="topbar""#,
        r#"class="tabstrip""#,
        r#"class="toast-layer""#,
        r#"class="statusbar""#,
    ] {
        assert_inert(&html, marker);
    }
    // The work area itself must stay live: it is the panel's own ancestor (the panel renders inside
    // the record pane), so inerting it would inert the panel too.
    let workarea = open_tag(&html, r#"class="workarea""#);
    assert!(
        !workarea.contains("inert"),
        "the work area holding the panel stays interactive, got {workarea:?}:\n{html}"
    );
    let app = open_tag(&html, r#"class="app has-explorer""#);
    assert!(
        !app.contains("inert"),
        "and so does `.app`, the panel's outermost ancestor, got {app:?}:\n{html}"
    );
}

#[test]
fn a_shell_with_no_panel_open_is_fully_interactive() {
    let html = render(shell_without_panel);
    assert!(
        html.contains(r#"class="tabstrip""#),
        "the shell rendered its regions:\n{html}"
    );
    assert!(
        !html.contains("inert"),
        "no region is inert while no panel is open:\n{html}"
    );
}
