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
    ActiveFilter, FilterDraft, FilterDraftError, PickerSelection, choice_options, filter_chip_label, filter_columns,
    filter_ops, listed_category,
};

use crate::app::AppCtx;
use crate::components::{Checkbox, Chip, SelectChoice, SelectInput, TextInput, record_picker};
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
/// clause, its × removes it.
#[component]
pub fn FilterBar(
    kind: ListedKind,
    filters: Signal<Vec<ActiveFilter>>,
    editing: Signal<Option<FilterEditing>>,
) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    let loc = data_loc();
    let mut chips = Vec::new();
    for active in filters.read().iter() {
        chips.push(filter_chip_label(&loc, kind, active));
    }
    rsx! {
        if let Some(target) = editing() {
            FilterEditor { key: "{target:?}", kind, target, filters, editing }
        }
        if !chips.is_empty() {
            div { class: "list-filters", role: "group", aria_label: "{chrome.list_filters()}",
                for (index, label) in chips.into_iter().enumerate() {
                    Chip {
                        key: "{index}-{label}",
                        label: label.clone(),
                        onactivate: move |()| editing.set(Some(FilterEditing::Clause(index))),
                        activate_label: chrome.filter_edit(&label),
                        ondelete: move |()| {
                            filters.write().remove(index);
                            editing.set(None);
                        },
                        delete_label: chrome.filter_delete(&label),
                    }
                }
            }
        }
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
    editing: Signal<Option<FilterEditing>>,
) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
    let loc = data_loc();
    let services = match try_consume_context::<AppCtx>() {
        Some(AppCtx::Ready(state)) => Some(state.services().clone()),
        _ => None,
    };
    let mut draft = use_signal(|| {
        let applied = match target {
            FilterEditing::Clause(index) => filters.peek().get(index).map(|active| active.draft.clone()),
            FilterEditing::New => None,
        };
        applied.unwrap_or_else(|| FilterDraft::new(kind))
    });
    let mut error = use_signal(|| None::<EditorError>);
    let mut commit = move |active: ActiveFilter| {
        let mut applied = filters.write();
        match target {
            FilterEditing::Clause(index) if index < applied.len() => applied[index] = active,
            FilterEditing::Clause(_) | FilterEditing::New => applied.push(active),
        }
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
            {value_editor(kind, &current, draft)}
            if let Some(message) = message {
                span { class: "field-error", role: "alert", "{message}" }
            }
            div { class: "filter-actions",
                button { class: "btn sm primary", r#type: "button", onclick: apply, "{chrome.filter_apply()}" }
                if let FilterEditing::Clause(index) = target {
                    button {
                        class: "btn sm",
                        r#type: "button",
                        onclick: move |_| {
                            if index < filters.peek().len() {
                                filters.write().remove(index);
                            }
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
fn value_editor(kind: ListedKind, current: &FilterDraft, draft: Signal<FilterDraft>) -> Element {
    match current.value_type(kind) {
        None => rsx! {},
        Some(ColumnType::Text | ColumnType::Date | ColumnType::Number) => {
            typed_values(current.op == FilterOp::Between, current, draft)
        }
        Some(ColumnType::Choice) => rsx! {
            ChoiceValues { kind, column: current.column.clone(), draft }
        },
        Some(ColumnType::Tags) => rsx! {
            TagValues { draft }
        },
        Some(ColumnType::Record) => rsx! {
            RecordValue { kind, column: current.column.clone(), draft }
        },
    }
}

/// One text field, or two for a *between* range.
fn typed_values(between: bool, current: &FilterDraft, mut draft: Signal<FilterDraft>) -> Element {
    let chrome = use_context::<ChromeCtx>().0;
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
    let mut names = BTreeMap::new();
    for tag in tags.read().iter().flatten() {
        names.insert(tag.id.clone(), tag.name.clone().unwrap_or_default());
    }
    let chosen = draft.read().values.clone();
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
    use_hook(move || {
        let held = draft.peek();
        if let Some(human_id) = held.values.first() {
            picker.state.write().selection = Some(PickerSelection {
                human_id: human_id.clone(),
                title: held.labels.get(human_id).cloned().unwrap_or_else(|| human_id.clone()),
            });
        }
    });
    use_effect(move || {
        let selection = picker.state.read().selection.clone();
        let mut current = draft.write();
        current.values.clear();
        current.labels.clear();
        if let Some(selection) = selection {
            current.labels.insert(selection.human_id.clone(), selection.title);
            current.values.push(selection.human_id);
        }
    });
    rsx! {
        {record_picker(loc, &picker)}
    }
}
