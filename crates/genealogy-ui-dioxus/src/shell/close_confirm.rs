//! The close-tab / quit confirm dialog (`⌘W`/`⌘Q` over unsaved work; PR1 §1.4).
//!
//! Renders only while [`NavState::pending_close`] is armed. Composes the shared [`Modal`]; mounted in
//! `shell/root.rs` beside the other overlays.
//!
//! Three actions, so unsaved work has an outcome other than losing it (issue #240): **Save** /
//! **Save all** hands the record to its own screen ([`NavState::save_then_close`] /
//! [`NavState::save_all_then_quit`]), **Discard** applies the close as it stands, **Cancel** backs out.
//! Save is offered only for work that can actually be saved — an invalid draft, or a create tab with
//! nothing typed into it, gets a disabled button and a line in the body saying why.
//!
//! Unsaved work comes in two shapes and the body has to be truthful about which: an unsaved **draft**
//! (nothing stored — closing discards the record) or an in-progress **edit** of a stored record
//! (closing discards only the changes). The tab kind picks the variant.

use dioxus::prelude::*;

use crate::components::{Button, ButtonVariant, Modal};
use crate::i18n::Chrome;
use crate::shell::ChromeCtx;
use crate::shell::nav_state::{CloseRequest, NavState, OpenTab};

/// The resolved copy of one confirm: the heading, the body, the records at stake (the quit confirm
/// lists them; a single-tab close names its one record in the body instead), the three action labels,
/// and — when the work cannot be saved — the reason, which both disables Save and shows in the body.
struct ConfirmCopy {
    /// The dialog heading.
    title: String,
    /// The body paragraphs, in reading order.
    body: Vec<String>,
    /// The records whose work is at stake, by tab label. Empty for a single-tab close.
    at_stake: Vec<String>,
    /// Why Save is unavailable, or `None` when it can run.
    blocked: Option<String>,
    /// The save action's label.
    save: String,
    /// The discard action's label.
    discard: String,
    /// The cancel action's label.
    cancel: String,
}

/// The close-tab/quit confirm dialog, rendered only while a close/quit is pending confirmation.
#[component]
pub fn CloseConfirmDialog() -> Element {
    let mut nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
    let Some(request) = *nav.pending_close.read() else {
        return rsx! {};
    };
    let copy = match request {
        CloseRequest::Tab(index) => tab_confirm_copy(&nav, &chrome.0, index),
        CloseRequest::Quit => quit_confirm_copy(&nav, &chrome.0),
    };
    let blocked = copy.blocked.is_some();
    let reason = match copy.blocked {
        Some(reason) => rsx! {
            p { class: "muted", "{reason}" }
        },
        None => rsx! {},
    };
    let at_stake = if copy.at_stake.is_empty() {
        rsx! {}
    } else {
        rsx! {
            ul { class: "stack",
                for label in copy.at_stake {
                    li { "{label}" }
                }
            }
        }
    };
    rsx! {
        Modal {
            title: copy.title,
            open: true,
            footer: rsx! {
                Button {
                    label: copy.cancel,
                    variant: ButtonVariant::Ghost,
                    onclick: move |_| nav.cancel_close(),
                }
                Button {
                    label: copy.discard,
                    onclick: move |_| nav.confirm_close(),
                }
                Button {
                    label: copy.save,
                    variant: ButtonVariant::Primary,
                    disabled: blocked,
                    onclick: move |_| match request {
                        CloseRequest::Tab(index) => nav.save_then_close(index),
                        CloseRequest::Quit => nav.save_all_then_quit(),
                    },
                }
            },
            for paragraph in copy.body {
                p { "{paragraph}" }
            }
            {at_stake}
            {reason}
        }
    }
}

/// The copy for closing the single tab at `index`: the body names that record, and the discard label
/// follows the tab kind — a draft is discarded whole, a stored record only loses its changes.
fn tab_confirm_copy(nav: &NavState, chrome: &Chrome, index: usize) -> ConfirmCopy {
    let is_draft = nav.records.read().get(index).is_some_and(OpenTab::is_draft);
    let label = tab_label(nav, chrome, index);
    let body = if is_draft {
        chrome.close_tab_confirm_body(&label)
    } else {
        chrome.close_tab_confirm_body_edits(&label)
    };
    let discard = if is_draft {
        chrome.close_tab_confirm_discard_draft()
    } else {
        chrome.close_tab_confirm_discard()
    };
    ConfirmCopy {
        title: chrome.close_tab_confirm_title(),
        body: vec![body],
        at_stake: Vec::new(),
        blocked: save_blocked(nav, chrome, index, &label),
        save: chrome.close_tab_confirm_save(),
        discard,
        cancel: chrome.close_tab_confirm_cancel(),
    }
}

/// The copy for quitting: the body lists every record holding unsaved work by its tab label, so the
/// choice is made over the actual list rather than "one or more". Save all is blocked by the first
/// record that cannot be saved, named.
fn quit_confirm_copy(nav: &NavState, chrome: &Chrome) -> ConfirmCopy {
    // A quit can be armed by either shape at once; the draft copy is the stronger warning (a whole
    // record is lost, not just an edit), so an open draft wins.
    let any_draft = nav.records.read().iter().any(OpenTab::is_draft);
    let body = if any_draft {
        chrome.quit_confirm_body()
    } else {
        chrome.quit_confirm_body_edits()
    };
    let mut at_stake = Vec::new();
    let mut blocked = None;
    for index in 0..nav.records.read().len() {
        if !nav.tab_has_unsaved(index) {
            continue;
        }
        let label = tab_label(nav, chrome, index);
        if blocked.is_none() {
            blocked = save_blocked(nav, chrome, index, &label);
        }
        at_stake.push(label);
    }
    ConfirmCopy {
        title: chrome.quit_confirm_title(),
        body: vec![body, chrome.quit_confirm_list_intro()],
        at_stake,
        blocked,
        save: chrome.quit_confirm_save_all(),
        discard: chrome.quit_confirm_discard_all(),
        cancel: chrome.quit_confirm_cancel(),
    }
}

/// Why the tab at `index` cannot be saved from the confirm, or `None` when it can: a parked edit that
/// is invalid names the record, and a tab with no parked buffer at all (a create form nothing has been
/// typed into) has nothing to save.
fn save_blocked(nav: &NavState, chrome: &Chrome, index: usize, label: &str) -> Option<String> {
    if nav.tab_is_savable(index) {
        return None;
    }
    let parked = nav
        .records
        .read()
        .get(index)
        .is_some_and(|tab| nav.has_unsaved(&tab.edit_key()));
    if parked {
        return Some(chrome.close_confirm_cannot_save(label));
    }
    Some(chrome.close_confirm_nothing_to_save(label))
}

/// The tab at `index` named as the tabstrip names it: a saved record's own label, a draft's localized
/// "New <entity>". An already-closed index falls back to an empty label.
fn tab_label(nav: &NavState, chrome: &Chrome, index: usize) -> String {
    let tab = nav.records.read().get(index).cloned();
    match tab {
        Some(OpenTab::Saved(record)) => record.label,
        Some(OpenTab::Draft(category)) => chrome.draft_tab_label(&chrome.rail_label(category.label_id())),
        None => String::new(),
    }
}
