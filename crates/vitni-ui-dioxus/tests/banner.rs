//! SSR assertions for the shell banner (#535, ADR 0051): while the workspace holds events this version
//! cannot read, a persistent `role="alert"` strip under the top bar says how many and what to do; with
//! none it renders nothing, and behind an open panel it is inert with the rest of the chrome.

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::shell::ChromeCtx;
use vitni_ui_dioxus::shell::banner::ShellBanner;
use vitni_ui_dioxus::shell::nav_state::NavState;

fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn banner(tag: &'static str, unreadable: usize) -> Element {
    use_context_provider(|| ChromeCtx(chrome(tag)));
    use_context_provider(NavState::new);
    rsx! {
        ShellBanner { unreadable }
    }
}

#[test]
fn no_unreadable_events_render_no_banner() {
    let html = render(|| banner("en", 0));
    assert!(!html.contains("shell-banner"), "{html}");
}

#[test]
fn one_unreadable_event_is_announced_with_what_to_do() {
    let html = render(|| banner("en", 1));
    assert!(html.contains(r#"class="shell-banner" role="alert""#), "{html}");
    assert!(
        html.contains("One event in this workspace could not be read, so its record shows only what came before it."),
        "{html}"
    );
    assert!(
        html.contains("update vitni; otherwise restore an earlier backup."),
        "{html}"
    );
}

#[test]
fn several_unreadable_events_are_counted() {
    let html = render(|| banner("en", 3));
    assert!(
        html.contains(
            "3 events in this workspace could not be read, so their records show only what came before them."
        ),
        "{html}"
    );
}

#[test]
fn the_banner_is_localized() {
    let html = render(|| banner("no", 2));
    assert!(
        html.contains("2 hendelser i dette arbeidsområdet kunne ikke leses"),
        "{html}"
    );
    assert!(html.contains("gjenopprett en tidligere sikkerhetskopi"), "{html}");
}

#[test]
fn the_banner_is_inert_behind_an_open_panel() {
    let html = render(|| {
        use_context_provider(|| ChromeCtx(chrome("en")));
        let mut nav = use_context_provider(NavState::new);
        use_hook(move || nav.open_panel(ScopeId(1)));
        rsx! {
            ShellBanner { unreadable: 1 }
        }
    });
    assert!(html.contains(r#"inert="true""#), "{html}");
    assert_eq!(
        html.matches(r#"aria-hidden="true""#).count(),
        2,
        "the banner itself is hidden, not only its icon: {html}"
    );
}
