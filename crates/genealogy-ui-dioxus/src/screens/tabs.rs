//! Shared tab-content panels, one per detail-screen tab that every aggregate renders the same way.
//! Each is a pure `fn(loc, data, …) -> Element` (SSR-testable without `AppCtx`, the `shared.rs`
//! idiom) parameterised only over the per-entity variance — the edit-form enum `E` and the
//! entity-specific dispatch callbacks — so the tab markup lives here once instead of being copied
//! across the twelve screen modules.

use genealogy_ui::HistoryEntryVm;

use super::prelude::*;

/// The History tab: the per-record audit timeline (who/when/why), each undoable entry carrying an
/// undo control. `on_undo` dispatches the pane's `XEdit::UndoAssertion` for an assertion id; pass
/// `None` for an aggregate with no retraction (Tag), which renders the timeline read-only.
pub fn history_panel(loc: &Localizer, entries: &[HistoryEntryVm], on_undo: Option<Callback<String>>) -> Element {
    if entries.is_empty() {
        return rsx! { EmptyState { symbol: "🕓".to_owned(), message: loc.history_empty() } };
    }
    let undo_text = loc.history_undo_short();
    let entries: Vec<HistoryEntry> = entries
        .iter()
        .map(|entry| HistoryEntry {
            when: entry.when.clone(),
            what: entry.what.clone(),
            who: entry.who.clone(),
            why: entry.why.clone(),
            assertion_id: entry.assertion_id.clone(),
            can_undo: entry.can_undo,
            undo_text: undo_text.clone(),
            undo_label: loc.history_undo_label(&entry.what),
        })
        .collect();
    rsx! {
        div { class: "section-note", "{loc.history_note()}" }
        HistoryTimeline {
            entries,
            onundo: move |assertion_id: String| {
                if let Some(on_undo) = on_undo {
                    on_undo.call(assertion_id);
                }
            },
        }
    }
}
