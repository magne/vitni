//! The Explorer's *Columns* and sort menus (ADR 0052 §7, #679): the toolbar controls that choose which
//! columns a row shows after its title, in what order, and how the list is sorted. Both edit the
//! caller's [`ListLayout`]; the Explorer evaluates it into its query and stores it per kind.
//!
//! Each menu hangs under its trigger in a `.menu-anchor`, over a click-away `.menu-scrim`, like the
//! tabstrip's new-record menu. One of the menus (these two and *Saved*) is open at a time ([`ListMenu`]); Escape
//! closes it, from the trigger as well as from inside the menu.

use std::path::Path;

use dioxus::prelude::*;
use vitni_app::{ConfigStore as _, FileConfigStore, SortDirection};
use vitni_ui::{ListLayout, column_toggles, layout_to_store, sort_columns};

use crate::shell::nav_state::NavState;
use crate::shell::{ChromeCtx, data_loc};

/// Which toolbar menu is open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ListMenu {
    /// The *Columns* menu.
    Columns,
    /// The sort menu.
    Sort,
    /// The *Saved* menu of saved searches.
    Saved,
}

/// Writes `layout` to the list's preferences in `dir` and to [`NavState::lists`], unless it is what is
/// stored already or the layout a saved search applied: applying a search never writes `[ui.lists.<kind>]`.
/// A layout that differs from the applied one is the user's own, so the applied layout is forgotten and
/// returning to it later is stored like any other choice.
pub fn persist_list_layout(
    mut nav: NavState,
    dir: &Path,
    layout: &ListLayout,
    mut applied: Signal<Option<ListLayout>>,
) {
    let kind = layout.kind;
    let stored = ListLayout::resolve(kind, nav.lists.peek().get(&kind));
    let preferences = layout_to_store(layout, &stored, applied.peek().as_ref());
    if applied.peek().as_ref().is_some_and(|saved| saved != layout) {
        applied.set(None);
    }
    let Some(preferences) = preferences else {
        return;
    };
    let store = FileConfigStore::for_workspace(dir.to_path_buf());
    if let Err(error) = store.store_list_preferences(kind, &preferences) {
        tracing::warn!(%error, "could not persist the list layout");
    }
    nav.lists.write().insert(kind, preferences);
}

/// The *Columns* trigger and, when open, its menu: a checkbox per column the kind can show, the shown
/// ones first in their order, each with a move-up and a move-down.
#[component]
pub fn ColumnsMenu(layout: Signal<ListLayout>, open: Signal<Option<ListMenu>>) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    let loc = data_loc();
    let shown = open() == Some(ListMenu::Columns);
    let toggles = column_toggles(&loc, &layout.read());
    let last = layout.read().columns.len().saturating_sub(1);
    let label = chrome.list_columns();
    rsx! {
        span { class: "menu-anchor", onkeydown: move |event| close_on_escape(&event, open),
            button {
                class: "btn sm",
                r#type: "button",
                title: "{chrome.list_columns_title()}",
                aria_haspopup: "dialog",
                aria_expanded: if shown { "true" } else { "false" },
                onclick: move |_| open.set(if shown { None } else { Some(ListMenu::Columns) }),
                "{label}"
            }
            if shown {
                Scrim { open }
                div {
                    class: "list-menu",
                    role: "dialog",
                    aria_label: "{label}",
                    for (index, toggle) in toggles.into_iter().enumerate() {
                        div { key: "{toggle.id}", class: "list-menu-column",
                            label { r#for: "list-column-{toggle.id}",
                                input {
                                    r#type: "checkbox",
                                    id: "list-column-{toggle.id}",
                                    checked: toggle.shown,
                                    onchange: move |_| layout.write().toggle(toggle.id),
                                }
                                span { "{toggle.label}" }
                            }
                            if toggle.shown {
                                MoveButton {
                                    layout,
                                    column: toggle.id,
                                    label: toggle.label.clone(),
                                    earlier: true,
                                    disabled: index == 0,
                                }
                                MoveButton {
                                    layout,
                                    column: toggle.id,
                                    label: toggle.label.clone(),
                                    earlier: false,
                                    disabled: index == last,
                                }
                            }
                        }
                    }
                }
            }
        }
    }
}

/// One shown column's move-up or move-down button, disabled at that end of the shown columns.
#[component]
fn MoveButton(
    layout: Signal<ListLayout>,
    column: &'static str,
    label: String,
    earlier: bool,
    disabled: bool,
) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    rsx! {
        button {
            class: "btn sm ghost",
            r#type: "button",
            aria_label: "{chrome.list_column_move(&label, earlier)}",
            disabled,
            onclick: move |_| layout.write().shift(column, earlier),
            if earlier { "↑" } else { "↓" }
        }
    }
}

/// The sort trigger, naming the current sort, and when open its menu: the `human_id` order, each
/// column the kind can be sorted by, then the direction.
#[component]
pub fn SortMenu(layout: Signal<ListLayout>, open: Signal<Option<ListMenu>>) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    let loc = data_loc();
    let shown = open() == Some(ListMenu::Sort);
    let sort = layout.read().sort.clone();
    let current = match &sort {
        Some(sort) => chrome.sort_column_label(&loc.list_column_label(&sort.column), sort.direction),
        None => chrome.sort_label(vitni_ui::RowSort::IdAsc),
    };
    let columns = sort_columns(&loc, layout.read().kind);
    let sorted_by = sort.as_ref().map(|sort| sort.column.clone());
    let direction = sort.as_ref().map(|sort| sort.direction);
    rsx! {
        span { class: "menu-anchor", onkeydown: move |event| close_on_escape(&event, open),
            button {
                class: "btn sm",
                r#type: "button",
                title: "{chrome.sort_order_title()}",
                aria_haspopup: "menu",
                aria_expanded: if shown { "true" } else { "false" },
                onclick: move |_| open.set(if shown { None } else { Some(ListMenu::Sort) }),
                "{current}"
            }
            if shown {
                Scrim { open }
                div {
                    class: "list-menu",
                    role: "menu",
                    aria_label: "{chrome.sort_order_title()}",
                    SortItem {
                        label: chrome.sort_by_id(),
                        checked: sorted_by.is_none(),
                        onpick: move |()| layout.write().sort_by(None),
                    }
                    for (id, label) in columns {
                        SortItem {
                            key: "{id}",
                            label,
                            checked: sorted_by.as_deref() == Some(id),
                            onpick: move |()| layout.write().sort_by(Some(id)),
                        }
                    }
                    div { class: "menu-sep", role: "separator" }
                    for way in [SortDirection::Ascending, SortDirection::Descending] {
                        SortItem {
                            key: "{way:?}",
                            label: chrome.sort_direction(way),
                            checked: direction == Some(way),
                            disabled: direction.is_none(),
                            onpick: move |()| layout.write().sort_direction(way),
                        }
                    }
                }
            }
        }
    }
}

/// One radio item of the sort menu: a check mark when chosen.
#[component]
pub(crate) fn SortItem(
    label: String,
    checked: bool,
    #[props(default)] disabled: bool,
    onpick: EventHandler<()>,
) -> Element {
    rsx! {
        button {
            class: "menu-item",
            role: "menuitemradio",
            r#type: "button",
            aria_checked: if checked { "true" } else { "false" },
            aria_disabled: if disabled { "true" } else { "false" },
            onclick: move |_| {
                if !disabled {
                    onpick.call(());
                }
            },
            span { class: "menu-check", aria_hidden: "true", if checked { "✓" } }
            span { "{label}" }
        }
    }
}

/// The click-away catcher behind an open menu.
#[component]
pub(crate) fn Scrim(open: Signal<Option<ListMenu>>) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    rsx! {
        button {
            class: "menu-scrim",
            r#type: "button",
            aria_label: "{chrome.close()}",
            onclick: move |_| open.set(None),
        }
    }
}

/// Closes the open menu on Escape, and keeps the key from the shell dispatcher, which would close an
/// overlay instead; with no menu open the key goes on.
pub(crate) fn close_on_escape(event: &KeyboardEvent, mut open: Signal<Option<ListMenu>>) {
    if event.key() == Key::Escape && open.peek().is_some() {
        event.prevent_default();
        event.stop_propagation();
        open.set(None);
    }
}
