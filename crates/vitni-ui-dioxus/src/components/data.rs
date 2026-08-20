//! Data display: a table, a selectable list row, and badges/chips.

use dioxus::prelude::*;

use crate::shell::ChromeCtx;

/// A data table with a header row. The caller supplies the body rows as `children` (`<tr>`s).
#[component]
pub fn Table(
    /// The already-localized accessible name, rendered as a visually-hidden `<caption>` so the table
    /// announces its purpose (WCAG 1.3.1 / U43).
    caption: String,
    /// The already-localized column headers. A trailing empty header marks the row-actions column; it
    /// is rendered with a visually-hidden "Actions" name (from [`ChromeCtx`]) rather than a nameless
    /// `<th>` (U43).
    headers: Vec<String>,
    /// The table body rows.
    children: Element,
) -> Element {
    let actions_label = try_consume_context::<ChromeCtx>().map(|chrome| chrome.0.table_actions());
    rsx! {
        table { class: "tbl",
            caption { class: "sr-only", "{caption}" }
            thead {
                tr {
                    for header in headers.iter() {
                        if header.is_empty() {
                            th {
                                span { class: "sr-only", {actions_label.clone().unwrap_or_default()} }
                            }
                        } else {
                            th { "{header}" }
                        }
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
    /// An optional element id (e.g. for `aria-activedescendant` wiring from a search input).
    #[props(default)]
    id: Option<String>,
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
    /// Fired on pointer-down, before `onclick`. A picker's result list uses this to
    /// `event.prevent_default()` so the row never steals focus from a search input first —
    /// `WebKitGTK` else blurs (and closes) the picker before the row's own click lands.
    #[props(default)]
    onmousedown: Option<EventHandler<MouseEvent>>,
    /// Fired on activation.
    onclick: EventHandler<MouseEvent>,
) -> Element {
    rsx! {
        button {
            class: if selected { "row sel" } else { "row" },
            role: "option",
            id,
            tabindex: "{tabindex}",
            aria_selected: if selected { "true" } else { "false" },
            onmounted: move |event| {
                if let Some(onmounted) = &onmounted {
                    onmounted.call(event);
                }
            },
            onmousedown: move |event| {
                if let Some(onmousedown) = &onmousedown {
                    onmousedown.call(event);
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

/// One header badge: its text, and the colour of a leading dot when the badge *is* about a colour.
/// [`DetailContainer`](crate::master_detail::DetailContainer) takes these rather than bare strings so
/// a screen can ask for the dot (a Tag's colour badge — `docs/mockups/tag.html:64`).
#[derive(Clone, PartialEq, Eq)]
pub struct BadgeSpec {
    /// The already-localized text.
    pub label: String,
    /// An optional CSS colour for the leading dot.
    pub dot_color: Option<String>,
}

impl BadgeSpec {
    /// A text-only badge.
    #[must_use]
    pub fn text(label: String) -> Self {
        Self { label, dot_color: None }
    }

    /// A badge with a leading dot in `dot_color`.
    #[must_use]
    pub fn with_dot(label: String, dot_color: String) -> Self {
        Self {
            label,
            dot_color: Some(dot_color),
        }
    }
}

/// A small inline label, optionally with a leading colour dot.
#[component]
pub fn Badge(
    /// The already-localized text.
    label: String,
    /// An optional CSS colour for a leading 8px dot (`docs/mockups/tag.html:64`).
    #[props(default)]
    dot_color: Option<String>,
) -> Element {
    rsx! {
        span { class: "badge",
            if let Some(dot_color) = dot_color {
                span { class: "dot", style: "width:8px;height:8px;border-radius:var(--r-pill);background:{dot_color}" }
            }
            "{label}"
        }
    }
}

/// An inline chip (a MUI-Chip subset): a label with an optional leading adornment (a colour dot or a
/// glyph icon), an optional trailing muted secondary id, and — when `ondelete` is set — a trailing
/// delete control (`×`). One control for every "something inside a chip" the screens need, so the
/// chip markup lives here once rather than being hand-rolled per call site.
#[component]
pub fn Chip(
    /// The already-localized text.
    label: String,
    /// An optional CSS colour for the leading dot.
    #[props(default)]
    dot_color: Option<String>,
    /// An optional leading glyph adornment (e.g. `"❝"`), shown when no `dot_color` is set.
    #[props(default)]
    icon: Option<String>,
    /// An optional trailing muted secondary id (e.g. a record's human id).
    #[props(default)]
    id_label: Option<String>,
    /// When set, the chip is deletable: a trailing `×` control fires this. `delete_label` is then
    /// required for its accessible name.
    #[props(default)]
    ondelete: Option<EventHandler<()>>,
    /// The already-localized accessible name for the delete control (used only with `ondelete`).
    #[props(default)]
    delete_label: Option<String>,
    /// An optional already-localized hover tooltip for the delete control.
    #[props(default)]
    delete_title: Option<String>,
) -> Element {
    rsx! {
        span { class: "chip",
            if let Some(dot_color) = dot_color {
                span { class: "dot", style: "background:{dot_color}" }
            } else if let Some(icon) = icon {
                span { class: "chip-icon", aria_hidden: "true", "{icon}" }
            }
            "{label}"
            if let Some(id_label) = id_label {
                span { class: "row-id", "{id_label}" }
            }
            if let Some(ondelete) = ondelete {
                button {
                    class: "chip-delete",
                    r#type: "button",
                    aria_label: delete_label,
                    title: delete_title,
                    onclick: move |_| ondelete.call(()),
                    "×"
                }
            }
        }
    }
}
