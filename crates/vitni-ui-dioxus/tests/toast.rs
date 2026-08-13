//! SSR-probe assertions for the shell's one notice layer (issue #208): [`NavState::notify`] /
//! [`NavState::notify_error`] raise a `Notice`, [`ShellToast`] renders it as `.toast-layer > .toast`
//! inside `main.workarea`, and [`NavState::expire_notice`] / [`NavState::dismiss_notice`] clear it — a
//! stale `seq` (a superseded or already-dismissed notice's own timer) must be a no-op, so one toast's
//! auto-dismiss can never kill its successor's. Same probe style as `close_confirm.rs`.

use std::rc::Rc;

use dioxus::prelude::*;
use unic_langid::LanguageIdentifier;
use vitni_ui_dioxus::components::ToastKind;
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::shell::ChromeCtx;
use vitni_ui_dioxus::shell::nav_state::NavState;
use vitni_ui_dioxus::shell::toast::ShellToast;

/// A chrome localizer for a single explicit language (deterministic for tests).
fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

/// Renders a probe component to an HTML string.
fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// Renders a probe and settles it, so `use_effect` bodies run — needed only to prove the auto-dismiss
/// effect body itself does not panic with no desktop window mounted (mirrors `close_confirm.rs`).
fn render_settled(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    for _ in 0..8 {
        vdom.render_immediate(&mut dioxus::core::NoOpMutations);
    }
    dioxus_ssr::render(&vdom)
}

/// The marker block for the pure `NavState` notice methods, independent of `ShellToast`.
fn notice_probe(nav: &NavState) -> Element {
    let notice = nav.notice.read().clone();
    let message = notice
        .as_ref()
        .map_or_else(|| "NONE".to_owned(), |notice| notice.message.clone());
    let kind = notice.as_ref().map_or("NONE", |notice| match notice.kind {
        ToastKind::Info => "INFO",
        ToastKind::Error => "ERROR",
    });
    let seq = notice.as_ref().map_or(0, |notice| notice.seq);
    rsx! {
        div { "NOTICE:{message}" }
        div { "KIND:{kind}" }
        div { "SEQ:{seq}" }
    }
}

fn info_notice() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.notify("Saved".to_owned()));
    rsx! {
        main { class: "workarea",
            ShellToast {}
        }
    }
}

#[test]
fn notify_renders_a_toast_in_the_layer_inside_the_work_area() {
    let html = render(info_notice);
    assert!(
        html.contains(r#"class="workarea""#),
        "the layer mounts inside the work area:\n{html}"
    );
    assert!(
        html.contains(r#"class="toast-layer""#),
        "the shell owns one positioned layer:\n{html}"
    );
    assert!(html.contains(r#"class="toast""#), "the toast surface itself:\n{html}");
    assert!(html.contains("Saved"), "the message renders:\n{html}");
    assert!(
        html.contains(r#"role="status""#),
        "an info toast is announced politely:\n{html}"
    );
    assert!(
        html.contains(r#"aria-live="polite""#),
        "not assertive, so it does not steal focus:\n{html}"
    );
    assert!(html.contains("Dismiss"), "the dismiss action renders:\n{html}");
}

fn error_notice() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.notify_error("Could not save".to_owned()));
    rsx! {
        main { class: "workarea",
            ShellToast {}
        }
    }
}

#[test]
fn notify_error_renders_an_error_toast() {
    let html = render(error_notice);
    assert!(
        html.contains(r#"class="toast error""#),
        "the error class carries the distinction:\n{html}"
    );
    assert!(
        html.contains(r#"role="alert""#),
        "an error toast is announced assertively:\n{html}"
    );
    assert!(html.contains(r#"aria-live="assertive""#), "…and interrupts:\n{html}");
    assert!(html.contains("Could not save"), "the message renders:\n{html}");
}

#[test]
fn the_auto_dismiss_effect_does_not_panic_without_a_desktop_window() {
    // Settling runs the effect body; a bare SSR probe mounts no `DesktopContext`, so
    // `ShellToast`'s guard must skip the `tokio::time::sleep` spawn — spawning one anyway would panic
    // with no tokio runtime driving this thread.
    let html = render_settled(info_notice);
    assert!(
        html.contains("Saved"),
        "the toast still renders once effects have run:\n{html}"
    );
}

fn expire_with_stale_seq() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.notify("first".to_owned());
        nav.notify("second".to_owned());
        // The first notice's seq (1) — stale now that the second (seq 2) is live.
        nav.expire_notice(1);
    });
    notice_probe(&nav)
}

#[test]
fn a_stale_seq_does_not_clear_a_newer_notice() {
    let html = render(expire_with_stale_seq);
    assert!(
        html.contains("NOTICE:second"),
        "a superseded notice's timer must not kill its successor's:\n{html}"
    );
}

fn expire_with_current_seq() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.notify("only".to_owned());
        nav.expire_notice(1);
    });
    notice_probe(&nav)
}

#[test]
fn the_current_seq_clears_the_notice() {
    let html = render(expire_with_current_seq);
    assert!(
        html.contains("NOTICE:NONE"),
        "the live notice's own seq clears it:\n{html}"
    );
}

fn expire_an_error_notice() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.notify_error("failed".to_owned());
        nav.expire_notice(1);
    });
    notice_probe(&nav)
}

#[test]
fn expiring_an_error_notice_is_a_no_op() {
    let html = render(expire_an_error_notice);
    assert!(
        html.contains("NOTICE:failed"),
        "errors are sticky — no timer clears them:\n{html}"
    );
}

fn dismiss_an_info_notice() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.notify("info".to_owned());
        nav.dismiss_notice();
    });
    notice_probe(&nav)
}

#[test]
fn dismiss_clears_an_info_notice() {
    let html = render(dismiss_an_info_notice);
    assert!(html.contains("NOTICE:NONE"), "Dismiss clears an info toast:\n{html}");
}

fn dismiss_an_error_notice() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.notify_error("bad".to_owned());
        nav.dismiss_notice();
    });
    notice_probe(&nav)
}

#[test]
fn dismiss_clears_an_error_notice() {
    let html = render(dismiss_an_error_notice);
    assert!(
        html.contains("NOTICE:NONE"),
        "Dismiss clears a sticky error toast too:\n{html}"
    );
}

fn second_notify_replaces_first() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.notify("first".to_owned());
        nav.notify("second".to_owned());
    });
    notice_probe(&nav)
}

#[test]
fn a_second_notify_replaces_the_first_with_a_fresh_seq() {
    let html = render(second_notify_replaces_first);
    assert!(html.contains("NOTICE:second"), "one toast — latest wins:\n{html}");
    assert!(
        html.contains("SEQ:2"),
        "the replacement carries a fresh, higher seq:\n{html}"
    );
}
