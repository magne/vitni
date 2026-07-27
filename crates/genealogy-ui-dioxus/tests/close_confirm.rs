//! SSR-probe assertions for the close-tab/quit confirm flow on [`NavState`] (PR1 §1.4): closing a
//! saved tab is immediate; closing a draft (or quitting with one open) arms the confirm dialog
//! instead of discarding it silently. Like `dock.rs`, each probe drives `NavState` in `use_hook` and
//! renders a small marker the test inspects.

use std::rc::Rc;

use dioxus::prelude::*;
use genealogy_ui::Category;
use genealogy_ui::RecordRef;
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::shell::ChromeCtx;
use genealogy_ui_dioxus::shell::close_confirm::CloseConfirmDialog;
use genealogy_ui_dioxus::shell::nav_state::NavState;
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

/// The marker block: open-tab count, whether the confirm is armed, and the quit ticket value.
fn probe(nav: &NavState) -> Element {
    let tabs = nav.records.read().len();
    let pending = if nav.pending_close.read().is_some() {
        "SOME"
    } else {
        "NONE"
    };
    let quit = *nav.quit_requested.read();
    rsx! {
        div { "TABS:{tabs}" }
        div { "PENDING:{pending}" }
        div { "QUIT:{quit}" }
    }
}

fn close_saved_tab_is_immediate() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record("I0001", "Ada"));
        nav.request_close_tab(0);
    });
    probe(&nav)
}

#[test]
fn closing_a_saved_tab_is_immediate_with_no_confirm() {
    let html = render(close_saved_tab_is_immediate);
    assert!(html.contains("TABS:0"), "the saved tab closes immediately:\n{html}");
    assert!(
        html.contains("PENDING:NONE"),
        "no confirm armed for a saved tab:\n{html}"
    );
}

fn close_draft_tab_arms_confirm() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_close_tab(0);
    });
    probe(&nav)
}

#[test]
fn closing_a_draft_tab_arms_the_confirm_instead_of_discarding() {
    let html = render(close_draft_tab_arms_confirm);
    assert!(html.contains("TABS:1"), "the draft survives until confirmed:\n{html}");
    assert!(html.contains("PENDING:SOME"), "the confirm dialog is armed:\n{html}");
}

fn confirm_closes_the_pending_draft() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_close_tab(0);
        nav.confirm_close();
    });
    probe(&nav)
}

#[test]
fn confirming_closes_the_pending_draft() {
    let html = render(confirm_closes_the_pending_draft);
    assert!(html.contains("TABS:0"), "confirming discards the draft:\n{html}");
    assert!(
        html.contains("PENDING:NONE"),
        "the confirm clears once applied:\n{html}"
    );
}

fn cancel_keeps_the_pending_draft() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_close_tab(0);
        nav.cancel_close();
    });
    probe(&nav)
}

#[test]
fn cancelling_keeps_the_pending_draft_open() {
    let html = render(cancel_keeps_the_pending_draft);
    assert!(html.contains("TABS:1"), "cancelling keeps the draft tab open:\n{html}");
    assert!(
        html.contains("PENDING:NONE"),
        "the confirm clears without closing anything:\n{html}"
    );
}

fn quit_with_no_draft_quits_immediately() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.request_quit());
    probe(&nav)
}

#[test]
fn quitting_with_nothing_unsaved_quits_immediately() {
    let html = render(quit_with_no_draft_quits_immediately);
    assert!(html.contains("QUIT:1"), "quit fires with nothing to lose:\n{html}");
    assert!(
        html.contains("PENDING:NONE"),
        "no confirm needed with nothing unsaved:\n{html}"
    );
}

fn quit_with_draft_arms_confirm() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_quit();
    });
    probe(&nav)
}

#[test]
fn quitting_with_an_open_draft_arms_the_confirm() {
    let html = render(quit_with_draft_arms_confirm);
    assert!(html.contains("QUIT:0"), "quit does not fire until confirmed:\n{html}");
    assert!(html.contains("PENDING:SOME"), "the confirm dialog is armed:\n{html}");
}

fn quit_confirmed_after_draft() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_quit();
        nav.confirm_close();
    });
    probe(&nav)
}

#[test]
fn confirming_a_pending_quit_fires_it() {
    let html = render(quit_confirmed_after_draft);
    assert!(html.contains("QUIT:1"), "confirming the quit fires it:\n{html}");
    assert!(
        html.contains("TABS:1"),
        "the draft tab itself is untouched by a quit:\n{html}"
    );
}

/// The confirm dialog, forced open for a pending draft-tab close.
fn dialog_open_for_draft() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_close_tab(0);
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn dialog_renders_the_close_tab_confirm_for_a_draft() {
    let html = render(dialog_open_for_draft);
    assert!(html.contains(r#"role="dialog""#), "confirm dialog role:\n{html}");
    assert!(html.contains("Close tab?"), "close-tab confirm title:\n{html}");
    assert!(html.contains("Cancel"), "cancel action label:\n{html}");
}

/// The confirm dialog, forced open for a pending quit.
fn dialog_open_for_quit() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.request_quit();
    });
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn dialog_renders_the_quit_confirm() {
    let html = render(dialog_open_for_quit);
    assert!(html.contains("Quit?"), "quit confirm title:\n{html}");
}

/// The confirm dialog with nothing pending.
fn dialog_closed() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    use_context_provider(NavState::new);
    rsx! {
        CloseConfirmDialog {}
    }
}

#[test]
fn dialog_renders_nothing_when_not_pending() {
    let html = render(dialog_closed);
    assert!(html.trim().is_empty(), "no pending close renders nothing:\n{html}");
}
