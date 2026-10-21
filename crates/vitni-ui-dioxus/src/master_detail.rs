//! The generic list + master-detail framework (Phase 5 PR3).
//!
//! Three reusable components compose every entity screen: [`MasterDetail`] lays out the two panes,
//! [`ListPane`] is a searchable, keyboard-operable `listbox`, and [`DetailContainer`] is a record
//! header plus a related-item tab strip. They are driven entirely by `vitni-ui` view-models
//! ([`RowVm`], [`TabItem`]) and already-localized strings, so adding an aggregate is a thin screen
//! that builds those — not a bespoke layout. Creation is reached from the shell top bar/tabstrip
//! (`⌘N`, the new-record menu), not a per-list `New` button.

use std::collections::HashMap;

use dioxus::prelude::*;
use vitni_ui::{ListWindow, ListedRows, RowSort, RowVm};

use crate::components::{Badge, BadgeSpec, ListRow, TabItem, Tabs, TextInput};
use crate::screens::DockedRecordDetail;
use crate::shell::nav_state::{NavState, PaneRole};
use crate::shell::roving::next_vertical;

/// The editor area: the active record's detail pane, plus a second docked pane when a record is
/// split beside it. The list lives in the shell-level [`Explorer`](crate::shell::explorer::Explorer)
/// now, not here.
///
/// When a record is docked ([`NavState::docked_record_ref`]) the layout becomes a two-column
/// `split-2` and a second `.detail.docked` pane renders the docked record beside the active one.
/// The primary detail pane is a drop target: dragging a record tab over it (while a tab drag is
/// live) highlights it, and dropping docks that record. Rendered without a [`NavState`] in context
/// (bare SSR framework tests), the split, highlight, and drop are all inert — a single pane.
#[component]
pub fn MasterDetail(
    /// The primary pane (typically a [`DetailContainer`], or a select-prompt placeholder).
    detail: Element,
) -> Element {
    let nav = try_consume_context::<NavState>();
    let docked = nav.and_then(|nav| nav.docked_record_ref());
    let mut hot = use_signal(|| false);
    let root_class = if docked.is_some() {
        "master-detail split-2"
    } else {
        "master-detail"
    };
    let detail_class = if hot() { "detail drop-target" } else { "detail" };
    rsx! {
        div { class: "{root_class}",
            section {
                class: "{detail_class}",
                "data-hook": "record-pane",
                ondragover: move |event| {
                    // `prevent_default` is required or the drop never fires; only while a tab drag
                    // is live, so ordinary content drags are unaffected.
                    if nav.is_some_and(|nav| nav.dragging_tab.peek().is_some()) {
                        event.prevent_default();
                        hot.set(true);
                    }
                },
                ondragleave: move |_| hot.set(false),
                ondrop: move |event| {
                    event.prevent_default();
                    hot.set(false);
                    if let Some(mut nav) = nav {
                        nav.complete_tab_drag();
                    }
                },
                {detail}
            }
            if docked.is_some() {
                section { class: "detail docked", "data-hook": "docked-pane", DockedRecordDetail {} }
            }
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
    /// The message of a list whose search or filters match nothing.
    pub no_match: String,
}

/// The already-localized strings for the list toolbar's sort-cycle button: its `title` plus one label
/// per [`RowSort`] state (the label the button shows in that state).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SortChrome {
    /// The button's `title` (its accessible name — "Change sort order").
    pub title: String,
    /// The label shown when sorting by id, ascending.
    pub id_asc: String,
    /// The label shown when sorting by title, ascending.
    pub name_asc: String,
    /// The label shown when sorting by title, descending.
    pub name_desc: String,
}

impl SortChrome {
    /// Resolves the sort-button strings from the renderer's chrome catalogue.
    #[must_use]
    pub fn resolve(chrome: &crate::i18n::Chrome) -> Self {
        Self {
            title: chrome.sort_order_title(),
            id_asc: chrome.sort_label(RowSort::IdAsc),
            name_asc: chrome.sort_label(RowSort::TitleAsc),
            name_desc: chrome.sort_label(RowSort::TitleDesc),
        }
    }

    /// The label to show for `sort`.
    #[must_use]
    pub fn label(&self, sort: RowSort) -> &str {
        match sort {
            RowSort::IdAsc => &self.id_asc,
            RowSort::TitleAsc => &self.name_asc,
            RowSort::TitleDesc => &self.name_desc,
        }
    }
}

/// A searchable, keyboard-operable entity list.
///
/// The toolbar carries a search box, the caller's `tools` (the Explorer's *Filters*, *Columns* and *Saved*
/// buttons) and its `sort` control (the Explorer's sort menu, Help's [`SortCycle`]); the caller's `bar`
/// (the Explorer's filter chips and editor) sits under it. The
/// body is a `listbox` of rows with roving focus (↑/↓ move the tab stop, Enter/Space activate the row).
/// The caller owns the `search`, `sort` and `selected` signals and evaluates them into `rows`: the
/// Explorer through the store's list query (ADR 0052 §4), Help over its topics in memory. Creating a
/// new record is reached from the shell top bar/tabstrip (`⌘N`, the new-record menu), not from this
/// pane.
///
/// A long list mounts only a window of its rows (#590, [`ListWindow`]), and `rows` may hold only some
/// pages of the result: the box is as tall as the whole result, a row whose page is not read yet waits
/// in its place, and `onmissing` asks for the pages the window needs.
#[component]
pub fn ListPane(
    /// The rows of the result read so far, under its total.
    rows: ReadSignal<ListedRows>,
    /// The search text.
    search: Signal<String>,
    /// The sort control, last in the toolbar.
    sort: Element,
    /// The selected row id.
    selected: Signal<Option<String>>,
    /// The localized chrome strings (labels, placeholder).
    chrome: ListChrome,
    /// Whether a search or filter narrows the list, so an empty result matched nothing rather than
    /// the list being empty.
    #[props(default)]
    narrowed: bool,
    /// Fired when a row is activated, with the activated row (e.g. to open a record tab).
    #[props(default)]
    onselect: Option<Callback<RowVm>>,
    /// Fired with the pages the mounted window needs and `rows` lacks.
    #[props(default)]
    onmissing: Option<Callback<Vec<usize>>>,
    /// Extra toolbar controls, between the search box and the sort control.
    #[props(default)]
    tools: Option<Element>,
    /// Content under the toolbar, above the rows.
    #[props(default)]
    bar: Option<Element>,
) -> Element {
    let empty = rows.read().is_empty();
    let message = if narrowed { &chrome.no_match } else { &chrome.empty };
    rsx! {
        div { class: "list-toolbar",
            TextInput {
                class: "filter",
                role: "searchbox",
                aria_label: "{chrome.filter_placeholder}",
                placeholder: "{chrome.filter_placeholder}",
                value: "{search}",
                oninput: move |event: FormEvent| search.set(event.value()),
            }
            {tools}
            {sort}
        }
        {bar}
        if empty {
            p { class: "empty", "{message}" }
        } else {
            ListRows { rows, label: chrome.list_label.clone(), selected, onselect, onmissing }
        }
    }
}

/// A sort button for a list held in memory: each press moves `sort` to the next [`RowSort`] in the
/// cycle, and the button names the current one.
#[component]
pub fn SortCycle(sort: Signal<RowSort>, chrome: SortChrome) -> Element {
    rsx! {
        button {
            class: "btn sm",
            r#type: "button",
            title: "{chrome.title}",
            onclick: move |_| {
                let next = sort().next();
                sort.set(next);
            },
            "{chrome.label(sort())}"
        }
    }
}

/// The scrolled `listbox` of a [`ListPane`]: mounts the rows of the current [`ListWindow`] plus the
/// roving tab stop, each at its own row offset inside a box as tall as the whole result, so the
/// scrollbar spans every row while the DOM holds a bounded few. A mounted row whose page is not read
/// yet is a busy placeholder, and the pages the window lacks go to `onmissing`.
///
/// Roving focus is window-aware: ↑/↓ to a row that is not mounted moves the window to it and the row
/// takes focus as it mounts ([`ListItem`]'s `pending` check).
#[component]
fn ListRows(
    rows: ReadSignal<ListedRows>,
    label: String,
    selected: Signal<Option<String>>,
    onselect: Option<Callback<RowVm>>,
    onmissing: Option<Callback<Vec<usize>>>,
) -> Element {
    let mut window = use_signal(ListWindow::default);
    let focused = use_signal(|| 0_usize);
    let nodes = use_signal(HashMap::<String, MountedEvent>::new);
    let pending = use_signal(|| None::<usize>);
    let listed = rows.read();
    let total = listed.total();
    let stop = focused().min(total.saturating_sub(1));
    let mounted = window().rendered(total, stop);
    // Re-run on every change to `rows`, not only when the missing set changes: a list read again from
    // the top drops the pages still in flight, and the window may need the very same pages again.
    // The caller's `onmissing` skips a page already being read.
    use_effect(move || {
        let missing = {
            let listed = rows.read();
            let total = listed.total();
            let stop = focused().min(total.saturating_sub(1));
            listed.missing_pages(&window().rendered(total, stop))
        };
        if let (false, Some(onmissing)) = (missing.is_empty(), onmissing) {
            onmissing.call(missing);
        }
    });
    rsx! {
        div {
            class: "list-rows",
            onscroll: move |event| {
                let next = ListWindow::at_scroll(
                    event.scroll_top(),
                    f64::from(event.scroll_height()),
                    rows.peek().total(),
                );
                if *window.peek() != next {
                    window.set(next);
                }
            },
            div {
                class: "list-window",
                role: "listbox",
                aria_label: "{label}",
                style: "height:calc({total} * var(--list-row-h))",
                onkeydown: move |event| step_focus(&event, rows, focused, window, nodes, pending),
                for index in mounted {
                    if let Some(row) = listed.row(index) {
                        ListItem {
                            key: "{row.id}",
                            index,
                            total,
                            stop,
                            focused,
                            row: row.clone(),
                            selected,
                            nodes,
                            pending,
                            onselect,
                        }
                    } else {
                        div {
                            key: "busy-{index}",
                            class: "row",
                            role: "option",
                            aria_busy: "true",
                            aria_selected: "false",
                            tabindex: if index == stop { "0" } else { "-1" },
                            style: "top:calc({index} * var(--list-row-h))",
                            aria_posinset: index + 1,
                            aria_setsize: total,
                        }
                    }
                }
            }
        }
    }
}

/// ↑/↓ in a windowed list: moves the tab stop, keeps the window over it, and focuses the row — now if
/// it is mounted, else once it mounts.
fn step_focus(
    event: &KeyboardEvent,
    rows: ReadSignal<ListedRows>,
    mut focused: Signal<usize>,
    mut window: Signal<ListWindow>,
    nodes: Signal<HashMap<String, MountedEvent>>,
    mut pending: Signal<Option<usize>>,
) {
    let listed = rows.peek();
    let total = listed.total();
    let current = focused.peek().min(total.saturating_sub(1));
    let Some(next) = next_vertical(&event.key(), current, total) else {
        return;
    };
    event.prevent_default();
    focused.set(next);
    let revealed = window.peek().revealing(next);
    if *window.peek() != revealed {
        window.set(revealed);
    }
    let node = listed.row(next).and_then(|row| nodes.peek().get(&row.id).cloned());
    if let Some(node) = node {
        spawn(async move {
            let _ = node.set_focus(true).await;
        });
    } else {
        pending.set(Some(next));
    }
}

/// One row in a [`ListPane`]: a [`ListRow`] at its offset in the whole list, wired to selection and
/// the roving-focus bookkeeping — its mounted node is kept under its row id while it is mounted, and
/// it takes focus on mount when ↑/↓ was waiting for its index. A click moves the tab stop to the row,
/// so the row holding focus is always mounted, however far the window scrolls from it.
#[component]
fn ListItem(
    index: usize,
    total: usize,
    stop: usize,
    focused: Signal<usize>,
    row: RowVm,
    selected: Signal<Option<String>>,
    nodes: Signal<HashMap<String, MountedEvent>>,
    pending: Signal<Option<usize>>,
    #[props(default)] onselect: Option<Callback<RowVm>>,
) -> Element {
    let id = row.id.clone();
    use_drop({
        let id = id.clone();
        move || {
            if let Ok(mut nodes) = nodes.try_write() {
                nodes.remove(&id);
            }
        }
    });
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
            style: "top:calc({index} * var(--list-row-h))",
            aria_posinset: index + 1,
            aria_setsize: total,
            onmounted: move |event: MountedEvent| {
                nodes.write().insert(id.clone(), event.clone());
                if *pending.peek() == Some(index) {
                    pending.set(None);
                    spawn(async move {
                        let _ = event.set_focus(true).await;
                    });
                }
            },
            onclick: move |_| {
                focused.set(index);
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
    /// The record's user-facing id (e.g. `I0001`), shown as a badge. `None` for a record that has no
    /// id to show (a Tag never renders its UUID — data-model §9).
    #[props(default)]
    id_label: Option<String>,
    /// Extra badges (e.g. a privacy tag, or a Tag's colour with its swatch dot).
    #[props(default)]
    badges: Vec<BadgeSpec>,
    /// An optional short avatar text (e.g. initials).
    #[props(default)]
    avatar: Option<String>,
    /// An optional colour-dot avatar (a CSS colour), shown instead of `avatar` text (e.g. a Tag's
    /// colour swatch).
    #[props(default)]
    avatar_color: Option<String>,
    /// Header extras placed in the badge row (e.g. the privacy restrictions in force).
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
    // The docked pane's own title is a subordinate `<h2>`, not a second `<h1>`: `PaneRole` is provided
    // only there (`DockedRecordDetail`), so the active/undocked pane's markup is unchanged (#279).
    let docked = try_consume_context::<PaneRole>() == Some(PaneRole::Docked);
    // A pane renders its `SidePanel`s as this container's *siblings*, so an open panel inerts the
    // container rather than the shell root — `.app` is the panel's own ancestor (#312). Every root this
    // container renders (the header, and `Tabs`'s own two) has to carry it: `inert` is per-element, and
    // this is a fragment, not one wrapper. `try_consume_context` because the framework tests render the
    // container with no shell in context.
    let behind_panel = try_consume_context::<NavState>().and_then(|nav| nav.panel_inert());
    rsx! {
        div { class: "detail-head", "data-hook": "detail-head", inert: behind_panel, aria_hidden: behind_panel,
            if let Some(color) = avatar_color {
                div { class: "avatar-lg", style: "background:transparent",
                    span { class: "dot", style: "width:28px;height:28px;border-radius:var(--r-pill);background:{color}" }
                }
            } else if let Some(avatar) = avatar {
                div { class: "avatar-lg", aria_hidden: "true", "{avatar}" }
            }
            div { class: "detail-id",
                if docked {
                    h2 { class: "detail-title", "{title}" }
                } else {
                    h1 { class: "detail-title", "{title}" }
                }
                if let Some(subtitle) = subtitle {
                    div { class: "detail-sub", "{subtitle}" }
                }
                div { class: "wrap", style: "margin-top:8px",
                    if let Some(id_label) = id_label {
                        Badge { label: id_label }
                    }
                    for badge in badges {
                        Badge { label: badge.label, dot_color: badge.dot_color }
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
            inert: behind_panel,
            {children}
        }
    }
}
