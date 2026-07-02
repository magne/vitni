//! The generic list + master-detail framework (Phase 5 PR3).
//!
//! Three reusable components compose every entity screen: [`MasterDetail`] lays out the two panes,
//! [`ListPane`] is a searchable, keyboard-operable `listbox`, and [`DetailContainer`] is a record
//! header plus a related-item tab strip. They are driven entirely by `genealogy-ui` view-models
//! ([`RowVm`], [`TabItem`]) and already-localized strings, so adding an aggregate is a thin screen
//! that builds those — not a bespoke layout. Creation is reached from the shell top bar/tabstrip
//! (`⌘N`, the new-record menu), not a per-list `New` button.

use dioxus::prelude::*;
use genealogy_ui::{ListQuery, RowVm, visible_rows};

use crate::components::{Badge, ListRow, TabItem, Tabs};
use crate::shell::focus_trap::keep_typing_local;
use crate::shell::roving::roving_vertical;

/// The two-pane master-detail layout: a list on the left, a detail pane on the right.
#[component]
pub fn MasterDetail(
    /// The left pane (typically a [`ListPane`]).
    list: Element,
    /// The right pane (typically a [`DetailContainer`], or a select-prompt placeholder).
    detail: Element,
) -> Element {
    rsx! {
        div { class: "master-detail",
            aside { class: "list", {list} }
            section { class: "detail", {detail} }
        }
    }
}

/// The already-localized chrome strings a [`ListPane`] needs.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListChrome {
    /// The `listbox`'s accessible name.
    pub list_label: String,
    /// The filter input's placeholder and accessible name.
    pub filter_placeholder: String,
    /// The empty-list message.
    pub empty: String,
}

/// A searchable, keyboard-operable entity list.
///
/// The toolbar carries a filter searchbox; the body is a `listbox` of rows with roving focus (↑/↓
/// move the tab stop, Enter/Space activate the row). The caller owns the `query` and `selected`
/// signals and supplies the localized [`ListChrome`]. Creating a new record is reached from the shell
/// top bar/tabstrip (`⌘N`, the new-record menu), not from this pane.
#[component]
pub fn ListPane(
    /// The rows to show (before the search filter, which this pane applies).
    rows: Vec<RowVm>,
    /// The live search state.
    query: Signal<ListQuery>,
    /// The selected row id.
    selected: Signal<Option<String>>,
    /// The localized chrome strings (labels, placeholder).
    chrome: ListChrome,
    /// Fired when a row is activated, with the activated row (e.g. to open a record tab).
    #[props(default)]
    onselect: Option<Callback<RowVm>>,
) -> Element {
    let visible = visible_rows(&rows, &query.read());
    let total = visible.len();
    let focused = use_signal(|| 0_usize);
    let nodes = use_signal(Vec::<Option<MountedEvent>>::new);
    let stop = focused().min(total.saturating_sub(1));
    rsx! {
        div { class: "list-toolbar",
            input {
                class: "filter",
                r#type: "text",
                role: "searchbox",
                aria_label: "{chrome.filter_placeholder}",
                placeholder: "{chrome.filter_placeholder}",
                value: "{query.read().query}",
                oninput: move |event| query.write().query = event.value(),
                onkeydown: move |event| keep_typing_local(&event),
            }
        }
        if visible.is_empty() {
            p { class: "empty", "{chrome.empty}" }
        } else {
            div { class: "list-rows", role: "listbox", aria_label: "{chrome.list_label}",
                onkeydown: move |event| roving_vertical(&event, focused, nodes, total),
                for (index , row) in visible.into_iter().enumerate() {
                    ListItem { index, stop, row, selected, nodes, onselect }
                }
            }
        }
    }
}

/// One row in a [`ListPane`]: a [`ListRow`] wired to selection and the roving-focus bookkeeping.
#[component]
fn ListItem(
    index: usize,
    stop: usize,
    row: RowVm,
    selected: Signal<Option<String>>,
    nodes: Signal<Vec<Option<MountedEvent>>>,
    #[props(default)] onselect: Option<Callback<RowVm>>,
) -> Element {
    let is_selected = selected().as_deref() == Some(row.id.as_str());
    let activated = row.clone();
    let id_label = row.display_id().to_owned();
    rsx! {
        ListRow {
            title: row.title,
            subtitle: row.subtitle,
            id_label,
            avatar: row.avatar,
            dot_color: row.dot_color,
            selected: is_selected,
            tabindex: if index == stop { 0 } else { -1 },
            onmounted: move |event| {
                let mut nodes = nodes.write();
                if nodes.len() <= index {
                    nodes.resize(index + 1, None);
                }
                nodes[index] = Some(event);
            },
            onclick: move |_| {
                selected.set(Some(activated.id.clone()));
                if let Some(onselect) = &onselect {
                    onselect.call(activated.clone());
                }
            },
        }
    }
}

/// A record header (title, subtitle, id, badges) plus a related-item tab strip.
///
/// The caller owns the `active` tab signal and renders the active tab's content as `children`.
#[component]
pub fn DetailContainer(
    /// The record's already-localized title.
    title: String,
    /// An optional already-localized subtitle (e.g. the vital summary + sex).
    #[props(default)]
    subtitle: Option<String>,
    /// The record's user-facing id (e.g. `I0001`), shown as a badge.
    id_label: String,
    /// Extra already-localized string badges (e.g. a privacy tag).
    #[props(default)]
    badges: Vec<String>,
    /// An optional short avatar text (e.g. initials).
    #[props(default)]
    avatar: Option<String>,
    /// Interactive header extras placed in the badge row (e.g. the restriction toggles).
    extras: Element,
    /// The right-aligned header actions (e.g. Edit / Compare).
    actions: Element,
    /// The detail tabs, in display order.
    tabs: Vec<TabItem>,
    /// The active tab index.
    active: Signal<usize>,
    /// The active tab's panel content.
    children: Element,
) -> Element {
    rsx! {
        div { class: "detail-head",
            if let Some(avatar) = avatar {
                div { class: "avatar-lg", aria_hidden: "true", "{avatar}" }
            }
            div { class: "detail-id",
                div { class: "detail-title", "{title}" }
                if let Some(subtitle) = subtitle {
                    div { class: "detail-sub", "{subtitle}" }
                }
                div { class: "wrap", style: "margin-top:8px",
                    Badge { label: id_label }
                    for badge in badges {
                        Badge { label: badge }
                    }
                    {extras}
                }
            }
            div { class: "head-actions", {actions} }
        }
        Tabs {
            tabs,
            active: active(),
            onselect: move |index| active.set(index),
            {children}
        }
    }
}
