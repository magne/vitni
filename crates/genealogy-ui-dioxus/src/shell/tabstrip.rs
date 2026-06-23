//! The in-app record tabstrip: the open record tabs plus an "open another" button.
//!
//! `⌘1…9` switches tabs (handled by the shell dispatcher); clicking a tab activates it, the `✕`
//! closes it. Drag-to-split docking of a tab into `.master-detail.split-2` is deferred to the
//! Compare/Merge slice (PR14), where a second pane has content; the CSS is already in place.

use dioxus::prelude::*;

use crate::shell::ChromeCtx;
use crate::shell::nav_state::{NavState, Overlay};

/// The open-records tab strip.
#[component]
pub fn RecordTabstrip() -> Element {
    let chrome = use_context::<ChromeCtx>();
    let mut nav = use_context::<NavState>();
    let tabs = nav.tabs.read().clone();
    let active = *nav.active_tab.read();
    rsx! {
        div { class: "tabstrip", role: "tablist", aria_label: "{chrome.0.aria_open_records()}",
            for (index , destination) in tabs.into_iter().enumerate() {
                {
                    let label = chrome.0.rail_label(destination.label_id());
                    let is_active = index == active;
                    rsx! {
                        button {
                            class: if is_active { "rtab active" } else { "rtab" },
                            role: "tab",
                            tabindex: if is_active { "0" } else { "-1" },
                            aria_selected: if is_active { "true" } else { "false" },
                            onclick: move |_| nav.activate_tab(index),
                            "{label}"
                            span {
                                class: "close",
                                role: "button",
                                aria_label: "{chrome.0.close_tab_label()}",
                                onclick: move |event| {
                                    event.stop_propagation();
                                    nav.close_tab(index);
                                },
                                "✕"
                            }
                        }
                    }
                }
            }
            button {
                class: "rtab add",
                r#type: "button",
                aria_label: "{chrome.0.new_tab_label()}",
                onclick: move |_| nav.overlay.set(Overlay::Palette),
                "+"
            }
        }
    }
}
