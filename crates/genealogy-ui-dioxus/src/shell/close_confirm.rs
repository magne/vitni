//! The close-tab / quit confirm dialog (`⌘W`/`⌘Q` over unsaved work; PR1 §1.4).
//!
//! Renders only while [`NavState::pending_close`] is armed. Composes the shared [`Modal`]; mounted in
//! `shell/root.rs` beside the other overlays.
//!
//! Unsaved work comes in two shapes and the body has to be truthful about which: an unsaved **draft**
//! (nothing stored — closing discards the record) or an in-progress **edit** of a stored record
//! (closing discards only the changes). The tab kind picks the variant.

use dioxus::prelude::*;

use crate::components::{Button, ButtonVariant, Modal};
use crate::shell::ChromeCtx;
use crate::shell::nav_state::{CloseRequest, NavState, OpenTab};

/// The close-tab/quit confirm dialog, rendered only while a close/quit is pending confirmation.
#[component]
pub fn CloseConfirmDialog() -> Element {
    let mut nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
    let Some(request) = *nav.pending_close.read() else {
        return rsx! {};
    };
    let (title, body, confirm_label, cancel_label) = match request {
        CloseRequest::Tab(index) => {
            let is_draft = nav.records.read().get(index).is_some_and(OpenTab::is_draft);
            let label = tab_label(&nav, &chrome, index);
            let body = if is_draft {
                chrome.0.close_tab_confirm_body(&label)
            } else {
                chrome.0.close_tab_confirm_body_edits(&label)
            };
            (
                chrome.0.close_tab_confirm_title(),
                body,
                chrome.0.close_tab_confirm_confirm(),
                chrome.0.close_tab_confirm_cancel(),
            )
        }
        CloseRequest::Quit => {
            // A quit can be armed by either shape at once; the draft copy is the stronger warning (a
            // whole record is lost, not just an edit), so an open draft wins.
            let any_draft = nav.records.read().iter().any(OpenTab::is_draft);
            let body = if any_draft {
                chrome.0.quit_confirm_body()
            } else {
                chrome.0.quit_confirm_body_edits()
            };
            (
                chrome.0.quit_confirm_title(),
                body,
                chrome.0.quit_confirm_confirm(),
                chrome.0.quit_confirm_cancel(),
            )
        }
    };
    rsx! {
        Modal {
            title,
            open: true,
            footer: rsx! {
                Button {
                    label: cancel_label,
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| nav.cancel_close(),
                }
                Button {
                    label: confirm_label,
                    variant: ButtonVariant::Primary,
                    onclick: move |_| nav.confirm_close(),
                }
            },
            p { "{body}" }
        }
    }
}

/// The tab at `index` named as the tabstrip names it: a saved record's own label, a draft's localized
/// "New <entity>". An already-closed index falls back to an empty label.
fn tab_label(nav: &NavState, chrome: &ChromeCtx, index: usize) -> String {
    let tab = nav.records.read().get(index).cloned();
    match tab {
        Some(OpenTab::Saved(record)) => record.label,
        Some(OpenTab::Draft(category)) => chrome.0.draft_tab_label(&chrome.0.rail_label(category.label_id())),
        None => String::new(),
    }
}
