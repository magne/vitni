//! Navigation chrome: a tab strip, a breadcrumb, and a status line.

use dioxus::prelude::*;

/// One tab in a [`Tabs`] strip.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TabItem {
    /// A stable id used for the tab/panel ARIA wiring.
    pub id: String,
    /// The visible, already-localized label.
    pub label: String,
    /// An optional count badge (e.g. number of related items).
    pub count: Option<usize>,
}

/// A tab strip over a single panel. Controlled: the active index is a prop and selection is
/// forwarded via `onselect`; the caller renders the active panel as `children`.
#[component]
pub fn Tabs(
    /// The tabs, in display order.
    tabs: Vec<TabItem>,
    /// The active tab index.
    active: usize,
    /// Fired with the index of a newly selected tab.
    onselect: EventHandler<usize>,
    /// The active tab's panel content.
    children: Element,
) -> Element {
    let panel_id = tabs
        .get(active)
        .map_or_else(String::new, |tab| format!("panel-{}", tab.id));
    rsx! {
        div { class: "tabs", role: "tablist",
            for (index , tab) in tabs.iter().enumerate() {
                button {
                    class: if index == active { "tab active" } else { "tab" },
                    role: "tab",
                    id: "tab-{tab.id}",
                    aria_selected: if index == active { "true" } else { "false" },
                    aria_controls: "panel-{tab.id}",
                    onclick: move |_| onselect.call(index),
                    "{tab.label}"
                    if let Some(count) = tab.count {
                        span { class: "tab-count", "{count}" }
                    }
                }
            }
        }
        div { class: "tab-body", role: "tabpanel", id: "{panel_id}", {children} }
    }
}

/// A breadcrumb trail; the final segment is rendered as the current location.
#[component]
pub fn Breadcrumb(
    /// The trail segments, root first. The last is shown emphasized.
    segments: Vec<String>,
) -> Element {
    let last = segments.len().saturating_sub(1);
    rsx! {
        div { class: "breadcrumb",
            for (index , segment) in segments.iter().enumerate() {
                if index > 0 {
                    span { class: "sep", "/" }
                }
                if index == last {
                    b { "{segment}" }
                } else {
                    span { "{segment}" }
                }
            }
        }
    }
}

/// The bottom status bar. A `contentinfo` landmark; the active record is announced politely.
#[component]
pub fn StatusLine(
    /// The currently active record label, if any.
    #[props(default)]
    active_record: Option<String>,
    /// Right-aligned status content.
    children: Element,
) -> Element {
    rsx! {
        div { class: "statusbar", role: "contentinfo",
            if let Some(active_record) = active_record {
                span { class: "active-record", aria_live: "polite", "{active_record}" }
            }
            span { class: "sb-right", {children} }
        }
    }
}
