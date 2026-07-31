//! Layout containers: card, side panel, modal, and empty state.

use dioxus::prelude::*;

use crate::components::button::IconButton;
use crate::shell::focus_trap::{DialogFocus, FocusGuard, focus_guard};

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
        button {
            class: "sidepanel-scrim",
            r#type: "button",
            aria_label: "{close_label}",
            onmousedown: move |event: MouseEvent| event.prevent_default(),
            onclick: move |event| onclose.call(event),
        }
        div { class: "sidepanel", role: "dialog", aria_modal: "true", aria_label: "{title}",
            div { class: "sp-head",
                h3 { "{title}" }
                span { class: "spacer" }
                IconButton { icon: "✕".to_owned(), label: close_label.clone(), onclick: move |event| onclose.call(event) }
            }
            div { class: "sp-body stack", {children} }
            div { class: "sp-foot", {footer} }
        }
    }
}

/// A dialog on the shared `.overlay` layer. Controlled like [`SidePanel`]: renders nothing unless
/// `open`, and a click on the backdrop scrim is forwarded via `onclose` — the dismiss-without-deciding
/// path, so the caller decides what that means (the close/quit confirm cancels). Focus is trapped
/// inside the dialog and restored to the control that opened it on close
/// ([`crate::shell::focus_trap`]).
#[component]
pub fn Modal(
    /// The already-localized dialog heading.
    title: String,
    /// Whether the dialog is shown.
    open: bool,
    /// The accessible name for the click-away scrim (already localized).
    close_label: String,
    /// Fired when the scrim is clicked.
    onclose: EventHandler<MouseEvent>,
    /// The dialog body.
    children: Element,
    /// The footer (e.g. confirm/cancel buttons).
    footer: Element,
) -> Element {
    if !open {
        return rsx! {};
    }
    rsx! {
        div { class: "overlay",
            button {
                class: "modal-scrim",
                r#type: "button",
                aria_label: "{close_label}",
                onmousedown: move |event: MouseEvent| event.prevent_default(),
                onclick: move |event| onclose.call(event),
            }
            div {
                class: "modal",
                role: "dialog",
                aria_modal: "true",
                aria_label: "{title}",
                tabindex: "-1",
                "data-focus-trap": "true",
                {focus_guard(FocusGuard::Leading)}
                div { class: "m-head", "{title}" }
                div { class: "m-body", {children} }
                div { class: "m-foot", {footer} }
                {focus_guard(FocusGuard::Trailing)}
                DialogFocus {}
            }
        }
    }
}
