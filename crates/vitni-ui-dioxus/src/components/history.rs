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
            for entry in entries.iter() {
                div { class: "tl-item",
                    div { class: "tl-when", "{entry.when}" }
                    div { class: "tl-what", "{entry.what}" }
                    div { class: "tl-who", "{entry.who}" }
                    if let Some(why) = &entry.why {
                        div { class: "tl-why", "{why}" }
                    }
                    if entry.can_undo {
                        div { class: "row-actions", style: "margin-top:4px",
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
            }
        }
    }
}
