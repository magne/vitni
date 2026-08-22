//! Layout containers: card, side panel, modal, and empty state.

use dioxus::prelude::*;

use crate::components::button::IconButton;
use crate::shell::focus_trap::{DialogFocus, FocusGuard, dismiss_on_escape, focus_guard};
use crate::shell::nav_state::NavState;

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
/// `onclose` — the ✕, a click on the scrim, and `Esc` all take that one path. Focus is trapped inside
/// the panel and restored to the control that opened it on close, the same cycling trap [`Modal`] uses
/// ([`crate::shell::focus_trap`]).
///
/// Unlike an overlay, a panel renders *inside* `.app`, so the shell cannot inert the background by
/// inerting `.app` — that is the panel's own ancestor, and `inert` cannot be undone by a descendant.
/// Instead the panel registers its scope in [`NavState::open_panels`] while it is open, and every
/// region behind it — the rail, Explorer, top bar, tabstrip, status bar, toast layer, skip link, and
/// the record container it is a sibling of — inerts its own root
/// ([`NavState::panel_inert`], #312). So the trap closes the keyboard path, the scrim the pointer one,
/// and `inert` the assistive-technology one.
///
/// Registration happens **during render**, not in an effect: `dioxus-ssr` runs no effects, and the
/// behaviour has to be assertable there. Unregistration is the load-bearing half — a missed one leaves
/// the whole shell inert, i.e. a frozen application — so it runs on both exits: the `open: false`
/// branch (the component stays mounted when a caller merely closes the panel) and a `use_drop` for the
/// live-renderer path where the pane drops the panel from its rsx.
///
/// `position: absolute`, escaping into the nearest positioned ancestor — a record's own `.detail`
/// pane for every `*DetailPane`'s side panel, bounding it to that pane rather than the whole window.
/// A panel mounted somewhere with no such ancestor (Geography's own geometry panel, the one
/// non-`.detail` caller) sets `viewport_anchored` instead, so it escapes all the way to the viewport
/// as before — `.workarea` is `position: relative` in its own right (the toast layer's containing
/// block, #208), and `.sidepanel` must never fall through to *that* by accident.
#[component]
pub fn SidePanel(
    /// The already-localized panel title.
    title: String,
    /// Whether the panel is shown.
    open: bool,
    /// The accessible name for the close control (already localized).
    close_label: String,
    /// Fired when the panel is closed — the ✕, the scrim, or `Esc`.
    onclose: EventHandler<()>,
    /// The panel body.
    children: Element,
    /// The footer (e.g. action buttons).
    footer: Element,
    /// Escapes all the way to the viewport instead of the nearest positioned ancestor — for a panel
    /// mounted outside a record's `.detail` pane (Geography's geometry panel).
    #[props(default)]
    viewport_anchored: bool,
) -> Element {
    // `try_consume_context`: a bare SSR probe of this component (`tests/side_panel.rs`,
    // `tests/components.rs`) renders it with no shell in context, and must keep working.
    let nav = try_consume_context::<NavState>();
    let scope = dioxus::core::current_scope_id();
    // Ahead of the `open` branch, like every hook here: a hook that only ran while the panel was open
    // would shift the hook order the next time it closed.
    use_drop(move || {
        if let Some(mut nav) = nav {
            nav.close_panel(scope);
        }
    });
    if let Some(mut nav) = nav {
        if open {
            nav.open_panel(scope);
        } else {
            nav.close_panel(scope);
        }
    }
    if !open {
        return rsx! {};
    }
    let scrim_class = if viewport_anchored {
        "sidepanel-scrim sidepanel-scrim-viewport"
    } else {
        "sidepanel-scrim"
    };
    let panel_class = if viewport_anchored {
        "sidepanel sidepanel-viewport"
    } else {
        "sidepanel"
    };
    rsx! {
        button {
            class: scrim_class,
            r#type: "button",
            aria_label: "{close_label}",
            onmousedown: move |event: MouseEvent| event.prevent_default(),
            onclick: move |_| onclose.call(()),
        }
        div {
            class: panel_class,
            role: "dialog",
            aria_modal: "true",
            aria_label: "{title}",
            tabindex: "-1",
            "data-focus-trap": "true",
            onkeydown: move |event: KeyboardEvent| dismiss_on_escape(&event, || onclose.call(())),
            {focus_guard(FocusGuard::Leading)}
            div { class: "sp-head",
                h3 { "{title}" }
                span { class: "spacer" }
                IconButton { icon: "✕".to_owned(), label: close_label.clone(), onclick: move |_: MouseEvent| onclose.call(()) }
            }
            div { class: "sp-body stack", {children} }
            div { class: "sp-foot", {footer} }
            {focus_guard(FocusGuard::Trailing)}
            DialogFocus {}
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
    /// Whether the dialog takes the wide shape (`.modal-wide`) — for content a prompt's width cannot
    /// hold, e.g. an image.
    #[props(default)]
    wide: bool,
    /// The accessible name for the click-away scrim (already localized).
    close_label: String,
    /// Fired when the dialog is dismissed without deciding — the scrim is clicked, or `Esc` is pressed.
    onclose: EventHandler<()>,
    /// The dialog body.
    children: Element,
    /// The footer (e.g. confirm/cancel buttons).
    footer: Element,
) -> Element {
    if !open {
        return rsx! {};
    }
    rsx! {
        div {
            class: "overlay",
            onkeydown: move |event: KeyboardEvent| dismiss_on_escape(&event, || onclose.call(())),
            button {
                class: "modal-scrim",
                r#type: "button",
                aria_label: "{close_label}",
                onmousedown: move |event: MouseEvent| event.prevent_default(),
                onclick: move |_| onclose.call(()),
            }
            div {
                class: if wide { "modal modal-wide" } else { "modal" },
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
