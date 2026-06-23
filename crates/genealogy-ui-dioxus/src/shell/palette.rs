//! The command palette (`⌘K`).
//!
//! PR2 ships a stub: it opens centered with focus on its input, contains `Tab`, and closes on `Esc`
//! or a click outside. Live search and command execution wire in a later PR.

use dioxus::prelude::*;

use crate::shell::ChromeCtx;
use crate::shell::focus_trap::{keep_typing_local, trap_tab};
use crate::shell::nav_state::{NavState, Overlay};

/// The command palette overlay, rendered only while [`Overlay::Palette`] is open.
#[component]
pub fn CommandPalette() -> Element {
    let mut nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
    if *nav.overlay.read() != Overlay::Palette {
        return rsx! {};
    }
    rsx! {
        div { class: "overlay", onclick: move |_| nav.close_overlay(),
            div {
                class: "palette",
                role: "dialog",
                aria_modal: "true",
                aria_label: "{chrome.0.palette_title()}",
                onclick: move |event| event.stop_propagation(),
                onkeydown: move |event| trap_tab(&event),
                div { class: "p-input",
                    input {
                        r#type: "text",
                        autofocus: true,
                        placeholder: "{chrome.0.palette_placeholder()}",
                        onkeydown: move |event| keep_typing_local(&event),
                    }
                }
                div { class: "p-row sel",
                    span { "{chrome.0.palette_hint()}" }
                    span { class: "p-kind", "—" }
                }
            }
        }
    }
}
