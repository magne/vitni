//! The close-tab / quit confirm dialog (`⌘W`/`⌘Q` over unsaved work; PR1 §1.4).
//!
//! Renders only while [`NavState::pending_close`] is armed. Composes the shared [`Modal`]; mounted in
//! `shell/root.rs` beside the other overlays.
//!
//! Dismissing the dialog without choosing — a click on its backdrop scrim, or `Esc` (which the shell
//! dispatcher routes through [`NavState::dismiss_topmost`]) — takes the **Cancel** path, so neither
//! ever discards the work.
//!
//! Three actions, so unsaved work has an outcome other than losing it (issue #240): **Save** /
//! **Save all** hands the record to its own screen ([`NavState::save_then_close`] /
//! [`NavState::save_all_then_quit`]), **Discard** applies the close as it stands, **Cancel** backs out.
//!
//! A single-tab close gates Save on that one record: an invalid edit, or a create tab with nothing
//! typed into it, gets a disabled button and a line in the body saying why. **Save all** is gated over
//! the whole set instead (issue #261) — it runs whenever *any* record at stake can be saved, saves
//! those, and marks the rest in the list as records it leaves open. Only a set where nothing can be
//! saved disables it, so one untouched `⌘N` draft cannot speak for the records beside it.
//!
//! Unsaved work comes in two shapes and the body has to be truthful about which: an unsaved **draft**
//! (nothing stored — closing discards the record) or an in-progress **edit** of a stored record
//! (closing discards only the changes). The tab kind picks the variant.

use dioxus::prelude::*;

use crate::components::{Button, ButtonVariant, Modal};
use crate::i18n::Chrome;
use crate::shell::ChromeCtx;
use crate::shell::nav_state::{CloseRequest, NavState, OpenTab};
use crate::shell::tab_label::tab_label;

/// The resolved copy of one confirm: the heading, the body, the records at stake (the quit confirm
/// lists them; a single-tab close names its one record in the body instead), the three action labels,
/// and one closing line — either the reason Save cannot run at all ([`Self::blocked`], which also
/// disables it) or a note about what running it will do ([`Self::note`], which does not).
struct ConfirmCopy {
    /// The dialog heading.
    title: String,
    /// The body paragraphs, in reading order.
    body: Vec<String>,
    /// The records whose work is at stake, by tab label. Empty for a single-tab close.
    at_stake: Vec<String>,
    /// Why Save is unavailable, or `None` when it can run. Its presence is what disables the button.
    blocked: Option<String>,
    /// A closing line about what Save *will* do, shown when nothing blocks it — the partial Save all,
    /// which keeps the records it can and leaves the rest open. Never disables the button.
    note: Option<String>,
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
    let reason = match copy.blocked.or(copy.note) {
        Some(line) => rsx! {
            p { class: "muted", "{line}" }
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
            close_label: chrome.0.dismiss(),
            onclose: move |()| nav.cancel_close(),
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
        note: None,
        save: chrome.close_tab_confirm_save(),
        discard,
        cancel: chrome.close_tab_confirm_cancel(),
    }
}

/// The copy for quitting: the body lists every record holding unsaved work by its tab label, so the
/// choice is made over the actual list rather than "one or more".
///
/// Save all is gated over the whole set, not record by record (issue #261). While one listed record
/// can be saved the button runs: the records it cannot save are marked in the list and the note says
/// they are left open. Only a set where nothing can be saved blocks it, and then the first record's
/// reason is the one shown.
fn quit_confirm_copy(nav: &NavState, chrome: &Chrome) -> ConfirmCopy {
    // A quit can be armed by either shape at once; the draft copy is the stronger warning (a whole
    // record is lost, not just an edit), so an open draft wins.
    let any_draft = nav.records.read().iter().any(OpenTab::is_draft);
    let body = if any_draft {
        chrome.quit_confirm_body()
    } else {
        chrome.quit_confirm_body_edits()
    };
    let mut unsaved = Vec::new();
    for index in 0..nav.records.read().len() {
        if nav.tab_has_unsaved(index) {
            unsaved.push((index, tab_label(nav, chrome, index), nav.tab_is_savable(index)));
        }
    }
    let any_savable = unsaved.iter().any(|(_, _, savable)| *savable);
    let mut at_stake = Vec::new();
    let mut blocked = None;
    for (index, label, savable) in unsaved {
        if savable {
            at_stake.push(label);
            continue;
        }
        if any_savable {
            at_stake.push(chrome.quit_confirm_item_blocked(&label));
        } else {
            blocked = blocked.or_else(|| save_blocked(nav, chrome, index, &label));
            at_stake.push(label);
        }
    }
    let note = any_savable.then(|| chrome.quit_confirm_leaves_open());
    ConfirmCopy {
        title: chrome.quit_confirm_title(),
        body: vec![body, chrome.quit_confirm_list_intro()],
        at_stake,
        blocked,
        note,
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
