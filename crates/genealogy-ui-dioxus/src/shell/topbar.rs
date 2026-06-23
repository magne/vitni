//! The top bar (`banner`): an active-record breadcrumb, the global search box, and the theme and
//! help controls.

use dioxus::prelude::*;

use crate::components::{Breadcrumb, Button, ButtonVariant, IconButton};
use crate::shell::ChromeCtx;
use crate::shell::focus_trap::keep_typing_local;
use crate::shell::nav_state::{NavState, Overlay};

/// The shell top bar.
#[component]
pub fn Topbar() -> Element {
    let chrome = use_context::<ChromeCtx>();
    let mut nav = use_context::<NavState>();
    let active_label = chrome.0.rail_label(nav.active.read().label_id());
    rsx! {
        header { class: "topbar", role: "banner",
            nav { class: "breadcrumb-wrap", aria_label: "{chrome.0.aria_breadcrumb()}",
                Breadcrumb { segments: vec![active_label] }
            }
            div { class: "search", role: "search",
                span { aria_hidden: "true", "🔍" }
                label { class: "sr-only", r#for: "global-search", "{chrome.0.search_label()}" }
                input {
                    id: "global-search",
                    r#type: "text",
                    placeholder: "{chrome.0.search_placeholder()}",
                    onkeydown: move |event| keep_typing_local(&event),
                }
                kbd { aria_hidden: "true", "⌘K" }
            }
            Button {
                label: chrome.0.list_new(),
                variant: ButtonVariant::Primary,
                small: true,
                onclick: move |_| tracing::debug!("new-record action: context-aware creation lands with the editing PR"),
            }
            IconButton {
                icon: "◐".to_owned(),
                label: chrome.0.aria_theme_toggle(),
                onclick: move |_| {
                    let next = nav.theme.peek().toggled();
                    nav.theme.set(next);
                },
            }
            IconButton {
                icon: "?".to_owned(),
                label: chrome.0.aria_help(),
                onclick: move |_| nav.overlay.set(Overlay::Help),
            }
        }
    }
}
