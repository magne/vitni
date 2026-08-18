//! The from-anywhere new-record category picker (`⌘N` while [`entity_category`] is `None` — issue
//! #300): the Dashboard, Help, and every tool have no category to create in, so
//! [`NavState::request_new`] raises this instead of silently doing nothing.
//!
//! Built on the shared [`Modal`] (the tabstrip `+`'s own [`NewRecordMenu`](super::tabstrip::NewRecordMenu)
//! is a lighter anchored dropdown that stays as it is; this is the dialog-shaped equivalent for when
//! there is no tabstrip to anchor under at all). Reuses [`Category::creatable`] for the listing and
//! [`NavState::request_new_for`] to act on the pick — plain buttons, not `role="menu"`: Tab/Enter
//! already work inside the modal's own focus trap, where a `menu` role would owe APG arrow-key
//! handling this dialog does not implement.
//!
//! [`entity_category`]: super::nav_state::entity_category

use dioxus::prelude::*;
use vitni_ui::Category;

use crate::components::Modal;
use crate::shell::ChromeCtx;
use crate::shell::nav_state::{NavState, Overlay};

/// The from-anywhere new-record category picker, rendered only while [`Overlay::NewRecord`] is open.
#[component]
pub fn NewRecordPicker() -> Element {
    let mut nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
    if *nav.overlay.read() != Overlay::NewRecord {
        return rsx! {};
    }
    rsx! {
        Modal {
            title: chrome.0.new_record_picker_title(),
            open: true,
            close_label: chrome.0.dismiss(),
            onclose: move |()| nav.close_overlay(),
            footer: rsx! {},
            div { class: "new-record-grid",
                for category in Category::creatable() {
                    button {
                        class: "menu-item",
                        r#type: "button",
                        onclick: move |_| {
                            nav.request_new_for(category);
                            nav.close_overlay();
                        },
                        span { aria_hidden: "true", "{category.icon()}" }
                        "{chrome.0.rail_label(category.label_id())}"
                    }
                }
            }
        }
    }
}
