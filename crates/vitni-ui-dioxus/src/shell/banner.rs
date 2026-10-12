//! The shell banner: a persistent warning under the top bar while the workspace holds events this
//! version cannot read (ADR 0051). Unlike a notice it has no dismiss and no timer — it stays until the
//! events read again or a backup replaces them.

use dioxus::prelude::*;

use crate::shell::ChromeCtx;
use crate::shell::nav_state::NavState;

/// The banner for `unreadable` unreadable events; renders nothing when there are none.
#[component]
pub fn ShellBanner(unreadable: usize) -> Element {
    let chrome = use_context::<ChromeCtx>();
    let nav = use_context::<NavState>();
    // Behind an open `SidePanel` the banner is inert with the rest of the chrome (#312).
    let behind_panel = nav.panel_inert();
    if unreadable == 0 {
        return rsx! {};
    }
    rsx! {
        div {
            class: "shell-banner",
            role: "alert",
            inert: behind_panel,
            aria_hidden: behind_panel,
            span { class: "shell-banner-icon", aria_hidden: "true", "⚠" }
            "{chrome.0.unreadable_events_banner(unreadable)}"
        }
    }
}
