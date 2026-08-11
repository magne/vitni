//! The in-app record tabstrip: back/forward history controls, the open record tabs, and the
//! new-record menu.
//!
//! `⌘1…9` switches tabs (handled by the shell dispatcher); clicking a tab activates it, the `✕`
//! closes it. The control row renders on every destination (including the Dashboard, where the tab
//! list is simply empty) so back/forward stay reachable. Each tab is an HTML5 drag source: dragging
//! it onto the detail pane docks the record side-by-side (`.master-detail.split-2`); `⌘⇧1…9` is the
//! keyboard equivalent. The tab currently docked carries the `docked` class.
//!
//! A tab holding unsaved work — a draft, or a saved record mid-edit
//! ([`NavState::tab_has_unsaved`]) — carries the `unsaved` class, a dot glyph, and an "unsaved
//! changes" accessible name, so `⌘W`'s confirm is never a surprise.

use dioxus::prelude::*;
use genealogy_ui::Category;

use crate::shell::ChromeCtx;
use crate::shell::nav_state::NavState;
use crate::shell::tab_label::tab_labels;

/// The open-records tab strip.
#[component]
pub fn RecordTabstrip() -> Element {
    let chrome = use_context::<ChromeCtx>();
    let mut nav = use_context::<NavState>();
    let records = nav.records.read().clone();
    let active = *nav.active_record.read();
    // One pass for the whole strip, hoisted above the loop: a draft's ordinal is its position among its
    // category's drafts, which no single tab knows.
    let labels = tab_labels(&nav, &chrome.0);
    let mut menu_open = use_signal(|| false);
    rsx! {
        div { class: "tabstrip", role: "tablist", aria_label: "{chrome.0.aria_open_records()}",
            button {
                class: "icon-btn",
                r#type: "button",
                aria_label: "{chrome.0.tab_back()}",
                disabled: !nav.can_back(),
                onclick: move |_| nav.history_back(),
                "‹"
            }
            button {
                class: "icon-btn",
                r#type: "button",
                aria_label: "{chrome.0.tab_forward()}",
                disabled: !nav.can_forward(),
                onclick: move |_| nav.history_forward(),
                "›"
            }
            // The tabs get their own scroller so the strip itself keeps `overflow` visible — an
            // overflow ancestor would clip the absolutely-positioned new-record menu below it. The
            // "+" (and its anchored menu) sit outside the scroller and stay visible however many
            // tabs are open.
            div { class: "tabs-scroll",
                for (index , tab) in records.into_iter().enumerate() {
                    {
                        let is_active = Some(index) == active;
                        let category = tab.category();
                        let is_draft = tab.is_draft();
                        let is_docked = tab.as_saved().is_some_and(|record| {
                            nav.docked_record
                                .read()
                                .as_ref()
                                .is_some_and(|(docked_category, docked_id)| {
                                    *docked_category == category && *docked_id == record.human_id
                                })
                        });
                        let is_unsaved = nav.tab_has_unsaved(index);
                        let mut class = String::from("rtab");
                        if is_active {
                            class.push_str(" active");
                        }
                        if is_docked {
                            class.push_str(" docked");
                        }
                        if is_draft {
                            class.push_str(" draft");
                        }
                        if is_unsaved {
                            class.push_str(" unsaved");
                        }
                        let label = labels.get(index).cloned().unwrap_or_default();
                        let human_id = tab.human_id().map(str::to_owned);
                        let close_label = chrome.0.close_tab_named(&label);
                        // Only an unsaved tab overrides its accessible name; a clean tab is named by
                        // its own text content, which reads better than any generated string.
                        let unsaved_name = if is_unsaved {
                            Some(chrome.0.tab_unsaved_named(&label))
                        } else {
                            None
                        };
                        rsx! {
                            button {
                                class,
                                role: "tab",
                                aria_label: unsaved_name,
                                // Only saved tabs are drag sources; a draft has no record to dock.
                                draggable: if human_id.is_some() { "true" } else { "false" },
                                tabindex: if is_active { "0" } else { "-1" },
                                aria_selected: if is_active { "true" } else { "false" },
                                onclick: move |_| nav.activate_record(index),
                                ondragstart: move |_| {
                                    if let Some(human_id) = &human_id {
                                        nav.begin_tab_drag(category, human_id);
                                    }
                                },
                                ondragend: move |_| nav.end_tab_drag(),
                                if is_unsaved {
                                    span { class: "unsaved-dot", aria_hidden: "true", "●" }
                                }
                                "{label}"
                                span {
                                    class: "close",
                                    role: "button",
                                    tabindex: "0",
                                    aria_label: "{close_label}",
                                    onclick: move |event| {
                                        event.stop_propagation();
                                        nav.request_close_tab(index);
                                    },
                                    onkeydown: move |event| {
                                        if event.key() == Key::Enter || event.key() == Key::Character(" ".to_owned()) {
                                            event.prevent_default();
                                            event.stop_propagation();
                                            nav.request_close_tab(index);
                                        }
                                    },
                                    "✕"
                                }
                            }
                        }
                    }
                }
            }
            span { class: "menu-anchor",
                button {
                    class: "rtab add",
                    r#type: "button",
                    aria_label: "{chrome.0.new_tab_label()}",
                    onclick: move |_| menu_open.set(!menu_open()),
                    "+"
                }
                NewRecordMenu { open: menu_open }
            }
        }
    }
}

/// The "+" new-record menu: one item per creatable category (rail order), rendered when `open` is
/// `true`. Picking a category navigates there and opens its create flow
/// ([`NavState::request_new_for`]).
///
/// The scrim (a viewport-covering click-away catcher) and the menu are **siblings** inside the
/// tabstrip's `.menu-anchor`, so the menu anchors under the "+" tab rather than the viewport; the menu
/// layers above the scrim (its higher `z-index`).
#[component]
pub fn NewRecordMenu(open: Signal<bool>) -> Element {
    let mut nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
    if !open() {
        return rsx! {};
    }
    rsx! {
        button {
            class: "menu-scrim",
            r#type: "button",
            aria_label: "{chrome.0.close_tab_label()}",
            onclick: move |_| open.set(false),
        }
        div {
            class: "new-record-menu",
            role: "menu",
            aria_label: "{chrome.0.new_tab_label()}",
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    event.prevent_default();
                    open.set(false);
                }
            },
            for category in Category::creatable() {
                button {
                    class: "menu-item",
                    role: "menuitem",
                    r#type: "button",
                    onclick: move |_| {
                        nav.request_new_for(category);
                        open.set(false);
                    },
                    span { aria_hidden: "true", "{category.icon()}" }
                    "{chrome.0.rail_label(category.label_id())}"
                }
            }
        }
    }
}
