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
}

/// A vertical audit timeline of change-log entries.
#[component]
pub fn HistoryTimeline(
    /// The entries, most recent first.
    entries: Vec<HistoryEntry>,
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
                }
            }
        }
    }
}
