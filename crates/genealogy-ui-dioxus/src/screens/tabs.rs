//! Shared tab-content panels, one per detail-screen tab that every aggregate renders the same way.
//! Each is a pure `fn(loc, data, …) -> Element` (SSR-testable without `AppCtx`, the `shared.rs`
//! idiom) parameterised only over the per-entity variance — the edit-form enum `E` and the
//! entity-specific dispatch callbacks — so the tab markup lives here once instead of being copied
//! across the twelve screen modules.

use genealogy_ui::HistoryEntryVm;

use super::prelude::*;

/// The Tags tab, shared by every aggregate that carries tags: an "add tag" action that opens the
/// screen's tag form (`editing.set(Some(add_form))`), then the applied tags as name + colour-dot
/// chips, each with a delete control that fires `on_remove` with the tag's id. Generic over the
/// screen's edit-form enum `E` (the add button is the only place the form type appears — mirrors
/// `row_actions_cell<E>`); the untag command is dispatched by the caller's `on_remove`. Tags are
/// referenced by name; their UUID is never rendered (data-model §9), and tags never retract — untag
/// is the only removal.
pub fn tags_panel<E: Clone + PartialEq + 'static>(
    loc: &Localizer,
    tags: &[TagRef],
    mut editing: Signal<Option<E>>,
    add_form: E,
    on_remove: Callback<String>,
) -> Element {
    let untag_title = loc.action_title("untag");
    rsx! {
        div { class: "tab-actions",
            Button {
                label: loc.action_label("add-tag"),
                variant: ButtonVariant::Default,
                onclick: move |_| editing.set(Some(add_form.clone())),
            }
        }
        if tags.is_empty() {
            EmptyState { message: loc.tab_empty() }
        } else {
            div { class: "wrap",
                for tag in tags.iter() {
                    {
                        let tag_id = tag.id.clone();
                        let untag_title = untag_title.clone();
                        rsx! {
                            Chip {
                                key: "{tag.id}",
                                label: tag.name.clone(),
                                dot_color: tag.color.clone(),
                                delete_label: loc.action_remove_tag_named(&tag.name),
                                delete_title: untag_title,
                                ondelete: move |()| on_remove.call(tag_id.clone()),
                            }
                        }
                    }
                }
            }
        }
    }
}

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
