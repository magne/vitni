//! The top bar (`banner`): an active-record breadcrumb, the global search box, and the theme and
//! help controls.

use dioxus::prelude::*;
use genealogy_ui::Destination;

use crate::components::{Breadcrumb, Button, ButtonVariant, IconButton};
use crate::shell::ChromeCtx;
use crate::shell::focus_trap::keep_typing_local;
use crate::shell::nav_state::{NavState, Overlay};

/// The shell top bar.
#[component]
pub fn Topbar() -> Element {
    let chrome = use_context::<ChromeCtx>();
    let mut nav = use_context::<NavState>();
    let active = *nav.active.read();
    let mut segments = vec![chrome.0.rail_label(active.label_id())];
    // Append the active record only while its own category is showing — otherwise the breadcrumb on
    // the dashboard (or another screen) would trail a record that screen isn't displaying.
    if let Some(record) = nav.active_record_ref()
        && active == Destination::Category(record.category)
    {
        segments.push(record.label);
    }
    rsx! {
        header { class: "topbar", role: "banner",
            nav { class: "breadcrumb-wrap", aria_label: "{chrome.0.aria_breadcrumb()}",
                Breadcrumb { segments }
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
                onclick: move |_| nav.request_new(),
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
