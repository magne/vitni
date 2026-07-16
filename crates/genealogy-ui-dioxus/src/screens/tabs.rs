//! Shared tab-content panels, one per detail-screen tab that every aggregate renders the same way.
//! Each is a pure `fn(loc, data, …) -> Element` (SSR-testable without `AppCtx`, the `shared.rs`
//! idiom) parameterised only over the per-entity variance — the edit-form enum `E` and the
//! entity-specific dispatch callbacks — so the tab markup lives here once instead of being copied
//! across the twelve screen modules.

use genealogy_ui::{CitationRefVm, HistoryEntryVm};

use super::prelude::*;

/// The Citations tab shared by every record's Citations tab: source (a link to the citation's source
/// when known) · page · [Backs] · confidence · evidence axes · a Detach action (ui-review Appendix A).
/// Generic over the screen's edit-form enum `E` for the actions cell (mirrors `row_actions_cell<E>`).
///
/// `show_backs` adds the "Backs" column — how many records the citation backs — for the record types
/// whose subject is plural (Person, Family); the singular-subject records (Event, Place, Media) omit
/// it (`show_backs: false`), per the canonical-shape rule.
pub fn citations_table<E: Clone + PartialEq + 'static>(
    loc: &Localizer,
    citations: &[CitationRefVm],
    show_backs: bool,
    onretract: Callback<(String, String, bool)>,
) -> Element {
    if citations.is_empty() {
        return rsx! { EmptyState { message: loc.tab_empty() } };
    }
    let mut headers = vec![loc.field_label("source"), loc.field_label("page")];
    if show_backs {
        headers.push(loc.field_label("backs"));
    }
    headers.push(loc.field_label("confidence"));
    headers.push(loc.field_label("evidence"));
    headers.push(String::new());
    rsx! {
        Table {
            headers,
            for citation in citations.iter() {
                tr {
                    td {
                        if let Some(source_id) = &citation.source_id {
                            RecordLink {
                                category: Category::Sources,
                                human_id: source_id.clone(),
                                label: citation.source.clone().unwrap_or_else(|| source_id.clone()),
                            }
                        } else {
                            {citation.source.clone().unwrap_or_else(|| citation.human_id.clone())}
                        }
                    }
                    td { class: "muted", {citation.page.clone().unwrap_or_else(|| "—".to_owned())} }
                    if show_backs {
                        td { class: "muted", "{citation.backs_count}" }
                    }
                    td {
                        if let (Some(level), Some(label)) = (citation.confidence, citation.confidence_label.clone()) {
                            ConfidenceBadge { level, label }
                        } else {
                            span { class: "muted", "—" }
                        }
                    }
                    td { class: "wrap",
                        for chip in citation.evidence_axes.iter() {
                            EvidenceAxisChip { axis: chip.axis, label: chip.label.clone() }
                        }
                    }
                    {row_actions_cell::<E>(
                        loc,
                        &citation.human_id,
                        None,
                        None,
                        citation.assertion_id.clone().map(|id| RowRetract { assertion_id: id, button_label: "detach", title: "detach-citation", detach: true }),
                        None,
                        onretract,
                    )}
                }
            }
        }
    }
}

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

/// A collection tab with its "+ add" action header (`record-editing.html` §8): a `.tab-actions` bar
/// holding a single Default button that opens `add_form` via the `editing` signal, above the tab's
/// `body`. Generic over the screen's edit-form enum `E` so no add-callback plumbing is needed — the
/// button just arms the side panel. `add_label_id` is the `action_label` id (e.g. `"add-fact"`,
/// `"attach-citation"`).
pub fn tab_with_add<E: Clone + PartialEq + 'static>(
    loc: &Localizer,
    add_label_id: &str,
    mut editing: Signal<Option<E>>,
    add_form: E,
    body: Element,
) -> Element {
    rsx! {
        div { class: "tab-actions",
            Button {
                label: loc.action_label(add_label_id),
                variant: ButtonVariant::Default,
                onclick: move |_| editing.set(Some(add_form.clone())),
            }
        }
        {body}
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
