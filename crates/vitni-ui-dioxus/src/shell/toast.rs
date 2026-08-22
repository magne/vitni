//! The shell's one notice layer (issue #208): a single positioned surface at the bottom of the work
//! area, replacing the 19 screen-local toasts a save/undo/error used to render inside its own pane —
//! each shifting the layout it was reporting on and vanishing silently on a tab switch. Every screen
//! now raises its confirmation/error through [`NavState::notify`]/[`NavState::notify_error`]; this is
//! the only place that renders one.
//!
//! Mounted in `shell/root.rs` inside `main.workarea`, beside `Workarea {}` — `.toast-layer` is
//! `position: absolute` over `.workarea`'s `position: relative` (`components.css`), so the toast never
//! adds a row to the shell grid.

use std::time::Duration;

use dioxus::prelude::*;

use crate::components::{Toast, ToastKind};
use crate::shell::ChromeCtx;
use crate::shell::nav_state::NavState;

/// How long an info notice stays up before [`NavState::expire_notice`] clears it. Errors get no timer
/// (sticky until [`NavState::dismiss_notice`]).
const NOTICE_TTL: Duration = Duration::from_secs(6);

/// The shell's notice layer: renders the live [`NavState::notice`] (if any) and arms its auto-dismiss.
///
/// Always renders the `.toast-layer` wrapper so its CSS position never depends on whether a notice is
/// live; the [`Toast`] inside is the one that is conditionally `visible`.
#[component]
pub fn ShellToast() -> Element {
    let mut nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
    let notice = nav.notice.read().clone();

    // Arms the auto-dismiss timer for a freshly-raised info notice. A spawned `tokio::time::sleep`
    // needs a reactor, which a bare SSR probe (no desktop window) does not run — guarded by
    // `has_desktop_window`, the same pattern `media_asset::use_media_asset_handler` uses (a runtime
    // check: `#[cfg(feature = "desktop")]` alone would not isolate this, since SSR tests compile with
    // `desktop` under `--all-features`).
    let seq = notice
        .as_ref()
        .filter(|notice| notice.kind == ToastKind::Info)
        .map(|notice| notice.seq);
    use_effect(use_reactive!(|seq| {
        let Some(seq) = seq else {
            return;
        };
        if !has_desktop_window() {
            return;
        }
        spawn(async move {
            tokio::time::sleep(NOTICE_TTL).await;
            nav.expire_notice(seq);
        });
    }));

    let visible = notice.is_some();
    let message = notice
        .as_ref()
        .map_or_else(String::new, |notice| notice.message.clone());
    let kind = notice.as_ref().map_or(ToastKind::Info, |notice| notice.kind);
    // Behind an open `SidePanel` the notice layer is inert too (#312): its Dismiss action is a button
    // sitting over the work area, outside the panel.
    let behind_panel = nav.panel_inert();
    rsx! {
        div { class: "toast-layer", inert: behind_panel, aria_hidden: behind_panel,
            Toast {
                visible,
                message,
                kind,
                action_label: Some(chrome.0.notice_dismiss()),
                onaction: move |_| nav.dismiss_notice(),
            }
        }
    }
}

/// Whether a real desktop window is mounted (a live [`dioxus::desktop::DesktopContext`]) — `false`
/// under a bare SSR probe, and always `false` when the crate is built without the `desktop` feature.
#[cfg(feature = "desktop")]
fn has_desktop_window() -> bool {
    try_consume_context::<dioxus::desktop::DesktopContext>().is_some()
}

/// SSR/no-webview stub: no window is ever mounted without the `desktop` feature.
#[cfg(not(feature = "desktop"))]
fn has_desktop_window() -> bool {
    false
}
