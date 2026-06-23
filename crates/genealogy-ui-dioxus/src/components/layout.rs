//! Layout containers: card, side panel, modal, and empty state.

use dioxus::prelude::*;

use crate::components::button::IconButton;

/// A titled content container.
#[component]
pub fn Card(
    /// An optional, already-localized section title.
    #[props(default)]
    title: Option<String>,
    /// The card's body.
    children: Element,
) -> Element {
    rsx! {
        div { class: "card",
            if let Some(title) = title {
                h3 { "{title}" }
            }
            {children}
        }
    }
}

/// A no-data placeholder: a symbol and a message.
#[component]
pub fn EmptyState(
    /// The symbol/glyph shown above the message; defaults to the empty-set sign.
    #[props(default = "∅".to_owned())]
    symbol: String,
    /// The already-localized message.
    message: String,
) -> Element {
    rsx! {
        div { class: "card empty",
            div { class: "big", "{symbol}" }
            "{message}"
        }
    }
}

/// A slide-in editor panel. Controlled: renders nothing unless `open`; closing is forwarded via
/// `onclose`. The dynamic focus trap lands with the keyboard layer (PR2); the dialog semantics are
/// here.
#[component]
pub fn SidePanel(
    /// The already-localized panel title.
    title: String,
    /// Whether the panel is shown.
    open: bool,
    /// The accessible name for the close control (already localized).
    close_label: String,
    /// Fired when the close control is activated.
    onclose: EventHandler<MouseEvent>,
    /// The panel body.
    children: Element,
    /// The footer (e.g. action buttons).
    footer: Element,
) -> Element {
    if !open {
        return rsx! {};
    }
    rsx! {
        div { class: "sidepanel", role: "dialog", aria_modal: "true", aria_label: "{title}",
            div { class: "sp-head",
                h3 { "{title}" }
                span { class: "spacer" }
                IconButton { icon: "✕".to_owned(), label: close_label, onclick: move |event| onclose.call(event) }
            }
            div { class: "sp-body stack", {children} }
            div { class: "sp-foot", {footer} }
        }
    }
}

/// A small centered dialog. Controlled like [`SidePanel`].
#[component]
pub fn Modal(
    /// The already-localized dialog heading.
    title: String,
    /// Whether the dialog is shown.
    open: bool,
    /// The dialog body.
    children: Element,
    /// The footer (e.g. confirm/cancel buttons).
    footer: Element,
) -> Element {
    if !open {
        return rsx! {};
    }
    rsx! {
        div { class: "modal", role: "dialog", aria_modal: "true", aria_label: "{title}",
            div { class: "m-head", "{title}" }
            div { class: "m-body", {children} }
            div { class: "m-foot", {footer} }
        }
    }
}
