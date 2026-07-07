//! The in-app record tabstrip: back/forward history controls, the open record tabs, and the
//! new-record menu.
//!
//! `⌘1…9` switches tabs (handled by the shell dispatcher); clicking a tab activates it, the `✕`
//! closes it. The control row renders on every destination (including the Dashboard, where the tab
//! list is simply empty) so back/forward stay reachable. Drag-to-split docking of a tab into
//! `.master-detail.split-2` is deferred to the Compare/Merge slice (PR14), where a second pane has
//! content; the CSS is already in place.

use dioxus::prelude::*;
use genealogy_ui::Category;

use crate::shell::ChromeCtx;
use crate::shell::nav_state::NavState;

/// The open-records tab strip.
#[component]
pub fn RecordTabstrip() -> Element {
    let chrome = use_context::<ChromeCtx>();
    let mut nav = use_context::<NavState>();
    let records = nav.records.read().clone();
    let active = *nav.active_record.read();
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
            for (index , record) in records.into_iter().enumerate() {
                {
                    let is_active = Some(index) == active;
                    rsx! {
                        button {
                            class: if is_active { "rtab active" } else { "rtab" },
                            role: "tab",
                            tabindex: if is_active { "0" } else { "-1" },
                            aria_selected: if is_active { "true" } else { "false" },
                            onclick: move |_| nav.activate_record(index),
                            "{record.label}"
                            span {
                                class: "close",
                                role: "button",
                                aria_label: "{chrome.0.close_tab_label()}",
                                onclick: move |event| {
                                    event.stop_propagation();
                                    nav.close_record(index);
                                },
                                "✕"
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
