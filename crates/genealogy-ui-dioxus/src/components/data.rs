//! Data display: a table, a selectable list row, and badges/chips.

use dioxus::prelude::*;

/// A data table with a header row. The caller supplies the body rows as `children` (`<tr>`s).
#[component]
pub fn Table(
    /// The already-localized column headers.
    headers: Vec<String>,
    /// The table body rows.
    children: Element,
) -> Element {
    rsx! {
        table { class: "tbl",
            thead {
                tr {
                    for header in headers.iter() {
                        th { "{header}" }
                    }
                }
            }
            tbody { {children} }
        }
    }
}

/// One selectable row in an entity list. Rendered as a real `<button>` with `role=option` so it is
/// keyboard-activatable; the parent list supplies `role=listbox`. Roving `tabindex` is added by the
/// keyboard layer (PR2).
#[component]
pub fn ListRow(
    /// The primary, already-localized title.
    title: String,
    /// An optional secondary line (dates, place, …).
    #[props(default)]
    subtitle: Option<String>,
    /// An optional trailing id (e.g. `I0042`).
    #[props(default)]
    id_label: Option<String>,
    /// An optional short avatar text (e.g. initials).
    #[props(default)]
    avatar: Option<String>,
    /// Whether this row is selected.
    #[props(default)]
    selected: bool,
    /// Fired on activation.
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: if selected { "row sel" } else { "row" },
            role: "option",
            aria_selected: if selected { "true" } else { "false" },
            onclick: move |event| onclick.call(event),
            if let Some(avatar) = avatar {
                span { class: "avatar", "{avatar}" }
            }
            span { class: "row-main",
                span { class: "row-title", "{title}" }
                if let Some(subtitle) = subtitle {
                    span { class: "row-sub", "{subtitle}" }
                }
            }
            if let Some(id_label) = id_label {
                span { class: "row-id", "{id_label}" }
            }
        }
    }
}

/// A small inline label.
#[component]
pub fn Badge(
    /// The already-localized text.
    label: String,
) -> Element {
    rsx! {
        span { class: "badge", "{label}" }
    }
}

/// An inline chip with an optional leading colour dot.
#[component]
pub fn Chip(
    /// The already-localized text.
    label: String,
    /// An optional CSS colour for the leading dot.
    #[props(default)]
    dot_color: Option<String>,
) -> Element {
    rsx! {
        span { class: "chip",
            if let Some(dot_color) = dot_color {
                span { class: "dot", style: "background:{dot_color}" }
            }
            "{label}"
        }
    }
}
