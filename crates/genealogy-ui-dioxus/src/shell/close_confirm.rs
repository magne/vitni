//! The close-tab / quit confirm dialog (`⌘W`/`⌘Q` on an unsaved draft; PR1 §1.4).
//!
//! Renders only while [`NavState::pending_close`] is armed. Composes the shared [`Modal`]; mounted in
//! `shell/root.rs` beside the other overlays.

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
            let label = draft_tab_label(&nav, &chrome, index);
            (
                chrome.0.close_tab_confirm_title(),
                chrome.0.close_tab_confirm_body(&label),
                chrome.0.close_tab_confirm_confirm(),
                chrome.0.close_tab_confirm_cancel(),
            )
        }
        CloseRequest::Quit => (
            chrome.0.quit_confirm_title(),
            chrome.0.quit_confirm_body(),
            chrome.0.quit_confirm_confirm(),
            chrome.0.quit_confirm_cancel(),
        ),
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

/// The draft label for the tab at `index` (`request_close_tab` only ever arms [`CloseRequest::Tab`]
/// for a draft, so this always resolves — an already-closed index falls back to an empty label).
fn draft_tab_label(nav: &NavState, chrome: &ChromeCtx, index: usize) -> String {
    let category = nav.records.read().get(index).map(OpenTab::category);
    category.map_or_else(String::new, |category| {
        chrome.0.draft_tab_label(&chrome.0.rail_label(category.label_id()))
    })
}
