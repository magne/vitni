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
/// keyboard-activatable (Enter/Space fire `onclick`); the parent list supplies `role=listbox` and
/// drives roving focus via `tabindex` + `onmounted` (so ↑/↓ can move the single tab stop).
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
    /// An optional short avatar text (e.g. initials). Ignored when `dot_color` is set.
    #[props(default)]
    avatar: Option<String>,
    /// An optional CSS colour for a leading dot avatar (tags), rendered instead of `avatar`.
    #[props(default)]
    dot_color: Option<String>,
    /// Whether this row is selected.
    #[props(default)]
    selected: bool,
    /// The roving tab index: `0` for the single focusable stop, `-1` for the rest. Defaults to `0`
    /// so a row is normally focusable when no roving group manages it.
    #[props(default = 0)]
    tabindex: i64,
    /// Fired with the row's mounted node so a roving list can pull DOM focus to it.
    #[props(default)]
    onmounted: Option<EventHandler<MountedEvent>>,
    /// Fired on activation.
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: if selected { "row sel" } else { "row" },
            role: "option",
            tabindex: "{tabindex}",
            aria_selected: if selected { "true" } else { "false" },
            onmounted: move |event| {
                if let Some(onmounted) = &onmounted {
                    onmounted.call(event);
                }
            },
            onclick: move |event| onclick.call(event),
            if let Some(dot_color) = dot_color {
                div { class: "avatar", style: "background:transparent", aria_hidden: "true",
                    span {
                        class: "dot",
                        style: "width:14px;height:14px;border-radius:var(--r-pill);background:{dot_color}",
                    }
                }
            } else if let Some(avatar) = avatar {
                div { class: "avatar", aria_hidden: "true", "{avatar}" }
            }
            div { class: "row-main",
                div { class: "row-title", "{title}" }
                if let Some(subtitle) = subtitle {
                    div { class: "row-sub", "{subtitle}" }
                }
            }
            if let Some(id_label) = id_label {
                div { class: "row-id", "{id_label}" }
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
