//! The Explorer's filter bar (ADR 0052 §2–3, #678): the toolbar's *Filters* button, one chip per applied
//! clause, and the editor that adds, edits and removes a clause.
//!
//! The editor sits in the list column's flow under the toolbar, not floated: the column clips what
//! overflows it, and a clause's controls need its full width. A clause is a column of the kind's
//! catalogue, an operator its value type takes, and a value editor per type — text and dates and
//! numbers typed (two for *between*), choices and tags ticked, a record picked. The editing state is
//! a [`FilterDraft`]; applying it puts an [`ActiveFilter`] in the caller's `filters`, which the
//! Explorer evaluates through the store's list query.

use std::collections::BTreeMap;

use dioxus::prelude::*;
use vitni_app::{ColumnType, FilterOp, ListedKind, record_target};
use vitni_ui::{
    ActiveFilter, FilterDraft, FilterDraftError, PickerSelection, StaleClauseVm, choice_options, filter_chip_label,
    filter_columns, filter_ops, listed_category,
};

use crate::app::AppCtx;
use crate::components::{Checkbox, Chip, SelectChoice, SelectInput, TextInput, record_picker};
use crate::i18n::Chrome;
use crate::screens::use_existing_picker;
use crate::services::{load_tags, resolve_list_filter};
use crate::shell::{ChromeCtx, data_loc};

/// Which clause the editor has open.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FilterEditing {
    /// A clause being added.
    New,
    /// The applied clause at this index.
    Clause(usize),
    /// The marked clause at this index of the marked clauses, to be replaced by a working one.
    Stale(usize),
}

/// The toolbar's *Filters* button: opens the editor on a new clause, or closes it, and counts the
/// applied clauses.
#[component]
pub fn FiltersButton(count: usize, editing: Signal<Option<FilterEditing>>) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    let open = editing().is_some();
    rsx! {
        button {
            class: "btn sm",
            r#type: "button",
            title: "{chrome.list_filters_title()}",
            aria_expanded: if open { "true" } else { "false" },
            aria_controls: "filter-editor",
            onclick: move |_| editing.set(if open { None } else { Some(FilterEditing::New) }),
            "{chrome.list_filters()}"
            if count > 0 {
                span { class: "badge", "{count}" }
            }
        }
    }
}

/// The editor, when a clause is open, then the applied clauses as chips: a chip's label reopens its
/// clause, its × removes it. A clause of a saved search that cannot run (ADR 0052 §10) is a marked chip
/// after them; editing it opens a new clause, which replaces it on *Apply*.
#[component]
pub fn FilterBar(
    kind: ListedKind,
    filters: Signal<Vec<ActiveFilter>>,
    stale: Signal<Vec<StaleClauseVm>>,
    editing: Signal<Option<FilterEditing>>,
) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    let loc = data_loc();
    let mut chips = Vec::new();
    for active in filters.read().iter() {
        chips.push(filter_chip_label(&loc, kind, active));
    }
    let marked: Vec<String> = stale.read().iter().map(|clause| clause.label.clone()).collect();
    rsx! {
        if let Some(target) = editing() {
            FilterEditor { key: "{target:?}", kind, target, filters, stale, editing }
        }
        if !chips.is_empty() || !marked.is_empty() {
            div { class: "list-filters", role: "group", aria_label: "{chrome.list_filters()}",
                for (index, label) in chips.into_iter().enumerate() {
                    Chip {
                        key: "{index}-{label}",
                        label: label.clone(),
                        onactivate: move |()| editing.set(Some(FilterEditing::Clause(index))),
                        activate_label: chrome.filter_edit(&label),
                        ondelete: move |()| {
                            remove_clause(FilterEditing::Clause(index), filters, stale);
                            editing.set(None);
                        },
                        delete_label: chrome.filter_delete(&label),
                    }
                }
                for (index, label) in marked.into_iter().enumerate() {
                    Chip {
                        key: "stale-{index}-{label}",
                        label: label.clone(),
                        stale: true,
                        onactivate: move |()| editing.set(Some(FilterEditing::Stale(index))),
                        activate_label: chrome.filter_edit(&label),
                        ondelete: move |()| {
                            remove_clause(FilterEditing::Stale(index), filters, stale);
                            editing.set(None);
                        },
                        delete_label: chrome.filter_delete(&label),
                    }
                }
            }
        }
    }
}

/// Removes the entry at `index` from `list`, ignoring an index that is already gone.
fn remove_at<T: 'static>(mut list: Signal<Vec<T>>, index: usize) {
    if index < list.peek().len() {
        list.write().remove(index);
    }
}

/// Removes the clause `target` names, applied or marked; a new clause names nothing yet.
pub fn remove_clause(target: FilterEditing, filters: Signal<Vec<ActiveFilter>>, stale: Signal<Vec<StaleClauseVm>>) {
    match target {
        FilterEditing::Clause(index) => remove_at(filters, index),
        FilterEditing::Stale(index) => remove_at(stale, index),
        FilterEditing::New => {}
    }
}

/// Puts the editor's applied `active` clause where `target` says: over the clause it edited, or appended,
/// replacing the marked clause it was opened on.
pub fn commit_clause(
    target: FilterEditing,
    mut filters: Signal<Vec<ActiveFilter>>,
    stale: Signal<Vec<StaleClauseVm>>,
    active: ActiveFilter,
) {
    let mut applied = filters.write();
    match target {
        FilterEditing::Clause(index) if index < applied.len() => applied[index] = active,
        FilterEditing::Clause(_) | FilterEditing::New => applied.push(active),
        FilterEditing::Stale(index) => {
            applied.push(active);
            remove_at(stale, index);
        }
    }
}

/// The draft the editor opens on: the applied clause being edited, a marked clause's column when the
/// catalogue still has it, otherwise a new clause over the kind's title column.
#[must_use]
pub fn opening_draft(
    kind: ListedKind,
    target: FilterEditing,
    filters: &[ActiveFilter],
    stale: &[StaleClauseVm],
) -> FilterDraft {
    match target {
        FilterEditing::Clause(index) => filters
            .get(index)
            .map_or_else(|| FilterDraft::new(kind), |active| active.draft.clone()),
        FilterEditing::Stale(index) => {
            let draft = FilterDraft::new(kind);
            match stale.get(index) {
                Some(clause) => draft.with_column(kind, &clause.column),
                None => draft,
            }
        }
        FilterEditing::New => FilterDraft::new(kind),
    }
}

/// Why the editor's clause was not applied.
#[derive(Debug, Clone, PartialEq, Eq)]
enum EditorError {
    /// An input does not read.
    Draft(FilterDraftError),
    /// The store refused it (a picked record that no longer exists), already localized.
    Store(String),
}

/// The clause editor: column, condition, the value editor the column's type needs, and *Apply*,
/// *Remove* (an applied clause) and *Cancel*. Escape cancels.
#[component]
fn FilterEditor(
    kind: ListedKind,
    target: FilterEditing,
    filters: Signal<Vec<ActiveFilter>>,
    stale: Signal<Vec<StaleClauseVm>>,
    editing: Signal<Option<FilterEditing>>,
) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    let loc = data_loc();
    let services = match try_consume_context::<AppCtx>() {
        Some(AppCtx::Ready(state)) => Some(state.services().clone()),
        _ => None,
    };
    let mut draft = use_signal(|| opening_draft(kind, target, &filters.peek(), &stale.peek()));
    let mut error = use_signal(|| None::<EditorError>);
    let mut commit = move |active: ActiveFilter| {
        commit_clause(target, filters, stale, active);
        editing.set(None);
    };
    let apply = move |_| {
        let current = draft.peek().clone();
        let active = match current.apply(kind) {
            Ok(active) => active,
            Err(draft_error) => return error.set(Some(EditorError::Draft(draft_error))),
        };
        let needs_lookup = current.value_type(kind) == Some(ColumnType::Record);
        match (needs_lookup, services.clone()) {
            (true, Some(services)) => {
                spawn(async move {
                    match resolve_list_filter(services, kind, active).await {
                        Ok(active) => commit(active),
                        Err(message) => error.set(Some(EditorError::Store(message))),
                    }
                });
            }
            (true, None) | (false, _) => commit(active),
        }
    };
    let current = draft.read().clone();
    let column_ty = kind.column(&current.column).map(|column| column.ty);
    let mut columns = Vec::new();
    for column in filter_columns(&loc, kind) {
        columns.push(SelectChoice {
            value: column.id.to_owned(),
            label: column.label,
        });
    }
    let mut ops = Vec::new();
    for (op, label) in column_ty.map(|ty| filter_ops(&loc, ty)).unwrap_or_default() {
        ops.push(SelectChoice {
            value: op.as_str().to_owned(),
            label,
        });
    }
    let message = error().map(|error| match error {
        EditorError::Draft(draft_error) => loc.filter_draft_error(draft_error),
        EditorError::Store(message) => message,
    });
    rsx! {
        section {
            class: "filter-editor",
            id: "filter-editor",
            role: "group",
            aria_label: "{chrome.filter_editor()}",
            onkeydown: move |event| {
                if event.key() == Key::Escape {
                    event.prevent_default();
                    editing.set(None);
                }
            },
            div { class: "field",
                label { r#for: "filter-column", "{chrome.filter_column()}" }
                SelectInput {
                    id: "filter-column",
                    options: columns,
                    selected: current.column.clone(),
                    onchange: move |event: FormEvent| {
                        let moved = draft.peek().clone().with_column(kind, &event.value());
                        draft.set(moved);
                        error.set(None);
                    },
                }
            }
            div { class: "field",
                label { r#for: "filter-op", "{chrome.filter_op()}" }
                SelectInput {
                    id: "filter-op",
                    options: ops,
                    selected: current.op.as_str().to_owned(),
                    onchange: move |event: FormEvent| {
                        if let Some(op) = FilterOp::parse(&event.value()) {
                            let changed = draft.peek().clone().with_op(op);
                            draft.set(changed);
                            error.set(None);
                        }
                    },
                }
            }
            {value_editor(&chrome, kind, &current, draft)}
            if let Some(message) = message {
                span { class: "field-error", role: "alert", "{message}" }
            }
            div { class: "filter-actions",
                button { class: "btn sm primary", r#type: "button", onclick: apply, "{chrome.filter_apply()}" }
                if target != FilterEditing::New {
                    button {
                        class: "btn sm",
                        r#type: "button",
                        onclick: move |_| {
                            remove_clause(target, filters, stale);
                            editing.set(None);
                        },
                        "{chrome.filter_remove()}"
                    }
                }
                button {
                    class: "btn sm ghost",
                    r#type: "button",
                    onclick: move |_| editing.set(None),
                    "{chrome.filter_cancel()}"
                }
            }
        }
    }
}

/// The value editor `current`'s column and operator need: none for *is empty*.
fn value_editor(chrome: &Chrome, kind: ListedKind, current: &FilterDraft, draft: Signal<FilterDraft>) -> Element {
    match current.value_type(kind) {
        None => rsx! {},
        Some(ColumnType::Text | ColumnType::Date | ColumnType::Number) => {
            typed_values(chrome, current.op == FilterOp::Between, current, draft)
        }
        Some(ColumnType::Choice) => rsx! {
            ChoiceValues { kind, column: current.column.clone(), draft }
        },
        Some(ColumnType::Tags) => rsx! {
            TagValues { draft }
        },
        Some(ColumnType::Record) => rsx! {
            RecordValue { key: "{current.column}", kind, column: current.column.clone(), draft }
        },
    }
}

/// One text field, or two for a *between* range.
fn typed_values(chrome: &Chrome, between: bool, current: &FilterDraft, mut draft: Signal<FilterDraft>) -> Element {
    let first_label = if between {
        chrome.filter_from()
    } else {
        chrome.filter_value()
    };
    rsx! {
        div { class: "field",
            label { r#for: "filter-first", "{first_label}" }
            TextInput {
                id: "filter-first",
                value: "{current.first}",
                oninput: move |event: FormEvent| draft.write().first = event.value(),
            }
        }
        if between {
            div { class: "field",
                label { r#for: "filter-second", "{chrome.filter_to()}" }
                TextInput {
                    id: "filter-second",
                    value: "{current.second}",
                    oninput: move |event: FormEvent| draft.write().second = event.value(),
                }
            }
        }
    }
}

/// Adds or removes `value` from the draft's chosen values, keeping `label` as its name.
fn toggle(mut draft: Signal<FilterDraft>, value: &str, label: &str, chosen: bool) {
    let mut current = draft.write();
    current.values.retain(|held| held != value);
    current.labels.remove(value);
    if chosen {
        current.values.push(value.to_owned());
        current.labels.insert(value.to_owned(), label.to_owned());
    }
}

/// A checkbox per value of a choice column.
#[component]
fn ChoiceValues(kind: ListedKind, column: String, draft: Signal<FilterDraft>) -> Element {
    let loc = data_loc();
    let options = choice_options(&loc, kind, &column);
    let chosen = draft.read().values.clone();
    rsx! {
        div { class: "filter-values",
            for option in options {
                Checkbox {
                    key: "{option.key}",
                    label: option.label.clone(),
                    name: "filter-value-{option.key}",
                    checked: chosen.contains(&option.key),
                    onchange: move |event: FormEvent| toggle(draft, &option.key, &option.label, event.checked()),
                }
            }
        }
    }
}

/// A checkbox per tag in the workspace, held by id and shown by name.
#[component]
fn TagValues(draft: Signal<FilterDraft>) -> Element {
    let services = match try_consume_context::<AppCtx>() {
        Some(AppCtx::Ready(state)) => Some(state.services().clone()),
        _ => None,
    };
    let tags = use_resource(move || {
        let services = services.clone();
        async move {
            match services {
                Some(services) => load_tags(services).await.unwrap_or_default(),
                None => Vec::new(),
            }
        }
    });
    let loaded = tags.read();
    let mut names = BTreeMap::new();
    for tag in loaded.iter().flatten() {
        names.insert(tag.id.clone(), tag.name.clone().unwrap_or_default());
    }
    let chosen = draft.read().values.clone();
    if loaded.is_some() {
        add_deleted_tags(&mut names, &draft.read(), &data_loc().filter_deleted_tag());
    }
    drop(loaded);
    rsx! {
        div { class: "filter-values",
            for (id, name) in names {
                Checkbox {
                    key: "{id}",
                    label: name.clone(),
                    name: "filter-value-{id}",
                    checked: chosen.contains(&id),
                    onchange: move |event: FormEvent| toggle(draft, &id, &name, event.checked()),
                }
            }
        }
    }
}

/// Adds a row for each chosen tag id that names no tag, labelled `deleted_label`, so the clause's dead
/// value is visible and can be unticked.
fn add_deleted_tags(names: &mut BTreeMap<String, String>, draft: &FilterDraft, deleted_label: &str) {
    for id in &draft.values {
        if !names.contains_key(id) {
            names.insert(id.clone(), deleted_label.to_owned());
        }
    }
}

/// What a record clause's draft holds when its editor opens.
#[derive(Debug, PartialEq, Eq)]
enum HeldRecord {
    /// No record.
    Nothing,
    /// A record that exists, held by its `human_id`.
    Picked(PickerSelection),
    /// A record that no longer exists, shown as this label; its id is not shown.
    Deleted(String),
}

/// Reads the record `draft` holds, which is gone when the draft marks it deleted.
fn held_record(draft: &FilterDraft) -> HeldRecord {
    let Some(human_id) = draft.values.first() else {
        return HeldRecord::Nothing;
    };
    let title = draft.labels.get(human_id).cloned().unwrap_or_else(|| human_id.clone());
    if draft.deleted_records.contains(human_id) {
        return HeldRecord::Deleted(title);
    }
    HeldRecord::Picked(PickerSelection {
        human_id: human_id.clone(),
        title,
    })
}

/// Writes the picker's `selection` into `draft`; with none, the draft goes back to `deleted_draft`
/// (the clause as it opened on a deleted record) so the dead record stays in its clause.
fn sync_record_pick(draft: &mut FilterDraft, selection: Option<PickerSelection>, deleted_draft: Option<&FilterDraft>) {
    if let (None, Some(original)) = (&selection, deleted_draft) {
        draft.values.clone_from(&original.values);
        draft.labels.clone_from(&original.labels);
        draft.deleted_records.clone_from(&original.deleted_records);
        return;
    }
    draft.values.clear();
    draft.labels.clear();
    draft.deleted_records.clear();
    if let Some(selection) = selection {
        draft.labels.insert(selection.human_id.clone(), selection.title);
        draft.values.push(selection.human_id);
    }
}

/// A record picker over the kind `column` refers to; the pick is held by its `human_id` until the
/// clause is applied, which resolves it to the record's id.
#[component]
fn RecordValue(kind: ListedKind, column: String, draft: Signal<FilterDraft>) -> Element {
    let Some(AppCtx::Ready(state)) = try_consume_context::<AppCtx>() else {
        return rsx! {};
    };
    let Some(target) = record_target(kind, &column) else {
        return rsx! {};
    };
    let loc = state.data_loc();
    let chrome = state.chrome();
    let category = listed_category(target);
    let mut picker = use_existing_picker(
        state.services().clone(),
        category,
        chrome.filter_value(),
        "filter-record".to_owned(),
        loc.picker_entity(category),
        Vec::new(),
    );
    let gone = use_hook(|| match held_record(&draft.peek()) {
        HeldRecord::Picked(selection) => {
            picker.state.write().selection = Some(selection);
            None
        }
        HeldRecord::Deleted(label) => Some((label, draft.peek().clone())),
        HeldRecord::Nothing => None,
    });
    let picked = picker.state.read().selection.is_some();
    let deleted = gone.as_ref().map(|(label, _)| label.clone());
    use_effect(move || {
        let selection = picker.state.read().selection.clone();
        let held = gone.as_ref().map(|(_, held)| held);
        sync_record_pick(&mut draft.write(), selection, held);
    });
    rsx! {
        if let (Some(label), false) = (deleted, picked) {
            p { class: "filter-deleted", "{label}" }
        }
        {record_picker(loc, &picker)}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn draft(value: &str, label: &str) -> FilterDraft {
        let mut draft = FilterDraft::new(ListedKind::Event);
        draft.values.push(value.to_owned());
        draft.labels.insert(value.to_owned(), label.to_owned());
        draft
    }

    #[test]
    fn a_deleted_record_is_held_as_its_label_not_its_id() {
        let mut gone = draft("0000-dead", "deleted record");
        gone.deleted_records.insert("0000-dead".to_owned());
        let held = held_record(&gone);
        assert_eq!(held, HeldRecord::Deleted("deleted record".to_owned()));
    }

    #[test]
    fn an_unchanged_deleted_record_stays_in_its_clause() {
        let mut gone = draft("0000-dead", "deleted record");
        gone.deleted_records.insert("0000-dead".to_owned());
        let mut current = gone.clone();
        sync_record_pick(&mut current, None, Some(&gone));
        assert_eq!(current, gone);
    }

    #[test]
    fn picking_replaces_a_deleted_record_and_clearing_restores_it() {
        let mut gone = draft("0000-dead", "deleted record");
        gone.deleted_records.insert("0000-dead".to_owned());
        let mut current = gone.clone();
        let pick = PickerSelection {
            human_id: "P0001".to_owned(),
            title: "Ada".to_owned(),
        };
        sync_record_pick(&mut current, Some(pick), Some(&gone));
        assert_eq!(current.values, vec!["P0001".to_owned()]);
        assert!(current.deleted_records.is_empty());
        sync_record_pick(&mut current, None, Some(&gone));
        assert_eq!(current, gone);
    }

    #[test]
    fn clearing_a_pick_with_no_deleted_record_empties_the_clause() {
        let mut current = draft("P0001", "Ada");
        sync_record_pick(&mut current, None, None);
        assert!(current.values.is_empty() && current.labels.is_empty());
    }

    #[test]
    fn an_existing_record_is_held_by_its_human_id() {
        let held = held_record(&draft("P0001", "Ada"));
        let selection = PickerSelection {
            human_id: "P0001".to_owned(),
            title: "Ada".to_owned(),
        };
        assert_eq!(held, HeldRecord::Picked(selection));
    }

    #[test]
    fn a_record_titled_like_the_deleted_label_is_still_picked() {
        let held = held_record(&draft("L0001", "deleted record"));
        let selection = PickerSelection {
            human_id: "L0001".to_owned(),
            title: "deleted record".to_owned(),
        };
        assert_eq!(held, HeldRecord::Picked(selection));
    }

    #[test]
    fn a_deleted_tag_gets_a_row_under_its_label() {
        let mut names = BTreeMap::from([("t1".to_owned(), "Kept".to_owned())]);
        let mut held = draft("t1", "Kept");
        held.values.push("gone".to_owned());
        add_deleted_tags(&mut names, &held, "deleted tag");
        assert_eq!(names.get("gone").map(String::as_str), Some("deleted tag"));
        assert_eq!(names.get("t1").map(String::as_str), Some("Kept"));
    }
}
