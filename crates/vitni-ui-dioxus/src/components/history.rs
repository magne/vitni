//! The audit-trail timeline — the event-sourced differentiator.

use dioxus::prelude::*;

/// One entry in a [`HistoryTimeline`]: who did what, when, and why. The screen layer feeds these
/// from the change-log query (PR5); the component is pure presentation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct HistoryEntry {
    /// When it happened (already-formatted, localized).
    pub when: String,
    /// What happened (already-localized summary).
    pub what: String,
    /// Who caused it (operator, optionally with confidence).
    pub who: String,
    /// An optional rationale ("why").
    pub why: Option<String>,
    /// The assertion this entry recorded — the undo target.
    pub assertion_id: String,
    /// Whether this entry can be undone (renders the undo control).
    pub can_undo: bool,
    /// The short visible undo-button text (e.g. `Undo`).
    pub undo_text: String,
    /// The already-localized accessible label for the undo control (e.g. `Undo: Name asserted`).
    pub undo_label: String,
    /// An import-run row's already-localized count (e.g. `4 changes`), shown muted beside `what`.
    pub count: Option<String>,
    /// An identity decision's already-localized assessment (e.g. `Matched at 97% · …`), shown muted
    /// beneath the rationale.
    pub evidence: Option<String>,
    /// An import-run row's own entries, folded behind a disclosure (ADR 0049 §3).
    pub changes: Option<RunChanges>,
}

/// The entries an import-run row folds, and the label of the disclosure that shows them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunChanges {
    /// The disclosure's already-localized text (e.g. `What it changed`).
    pub label: String,
    /// The run's entries on this record, newest first. They carry no undo: the row's own undo
    /// retracts the run's newest assertion (#306).
    pub entries: Vec<HistoryEntry>,
}

/// A vertical audit timeline of change-log entries; undoable entries carry an undo control.
#[component]
pub fn HistoryTimeline(
    /// The entries, most recent first.
    entries: Vec<HistoryEntry>,
    /// Invoked with an entry's `assertion_id` when its undo control is activated.
    onundo: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "timeline",
            for (index, entry) in entries.into_iter().enumerate() {
                HistoryItem { key: "{index}-{entry.assertion_id}", entry, onundo }
            }
        }
    }
}

/// One timeline row; an import-run row also owns whether its changes are shown.
///
/// Keyed by position as well as assertion: a supersession and its replacement are one command, so
/// they share an `assertion_id`, and keyed siblings must be unique.
#[component]
fn HistoryItem(entry: HistoryEntry, onundo: EventHandler<String>) -> Element {
    let mut open = use_signal(|| false);
    rsx! {
        div { class: "tl-item",
            div { class: "tl-when", "{entry.when}" }
            div { class: "tl-what",
                "{entry.what}"
                if let Some(count) = &entry.count {
                    span { class: "muted tl-count", "{count}" }
                }
            }
            div { class: "tl-who", "{entry.who}" }
            if let Some(why) = &entry.why {
                div { class: "tl-why", "{why}" }
            }
            if let Some(evidence) = &entry.evidence {
                div { class: "tl-evidence muted", "{evidence}" }
            }
            if entry.can_undo || entry.changes.is_some() {
                div { class: "row-actions", style: "margin-top:4px",
                    if let Some(changes) = &entry.changes {
                        button {
                            class: "btn sm ghost",
                            r#type: "button",
                            aria_expanded: if open() { "true" } else { "false" },
                            onclick: move |_| open.toggle(),
                            if open() { "▾ {changes.label}" } else { "▸ {changes.label}" }
                        }
                    }
                    if entry.can_undo {
                        button {
                            class: "btn sm ghost",
                            r#type: "button",
                            "aria-label": "{entry.undo_label}",
                            onclick: {
                                let assertion_id = entry.assertion_id.clone();
                                move |_| onundo.call(assertion_id.clone())
                            },
                            "↩ {entry.undo_text}"
                        }
                    }
                }
            }
            if let Some(changes) = entry.changes.as_ref().filter(|_| open()) {
                div { class: "timeline tl-children", "data-hook": "run-changes",
                    for (index, change) in changes.entries.iter().cloned().enumerate() {
                        HistoryItem { key: "{index}-{change.assertion_id}", entry: change, onundo }
                    }
                }
            }
        }
    }
}
