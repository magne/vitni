//! The record picker and nested draft card (`record-editing.html` §6b, `design-system.html`).
//!
//! A record link is a find-or-create control: [`record_picker`] draws a search input over the
//! already-loaded options ([`PickerOptions`]), an in-flow result list of reused [`ListRow`]s (never a
//! floater — the `.detail` scroll container clips those), a trailing "+ New …" row, and a collapsed
//! [`.picker-value`](picker_value) chip once a record is picked. Picking "+ New" flips the call site's
//! [`RecordLink`](genealogy_ui::RecordLink) to `New(..)`, which renders a [`draft_card`] whose body may
//! hold another picker (a citation → source cascade); discarding the card resets the link to `Empty`.
//!
//! Every fn here is pure over signals (no `use_*` hooks, no `AppCtx`), so a call site can render a
//! picker conditionally and the SSR tests can exercise the markup without an app. The filtering lives
//! in [`genealogy_ui::picker_rows`]; this module is only the framework binding.

use dioxus::prelude::*;
use genealogy_ui::{Localizer, PickerSelection, PickerState, RowVm, picker_rows};

use crate::components::{IconButton, ListRow, TextInput};

/// The already-localized configuration of one picker: its field label, the element-id base, the entity
/// noun (`person`/`place`/…) used in the placeholder and "+ New" row, and whether "+ New" is offered.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PickerConfig {
    /// The field's already-localized label.
    pub label: String,
    /// The field's machine name / element id.
    pub name: String,
    /// The already-localized entity noun the picker searches for (from `Localizer::picker_entity`).
    pub entity_label: String,
    /// Whether the picker offers a "+ New …" create row (a find-or-create picker vs existing-only).
    pub allow_new: bool,
}

/// The load state of a picker's options, loaded once per open form via `load_picker_rows`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PickerOptions {
    /// The options are still loading.
    Loading,
    /// Loading failed with an already-localized message.
    Failed(String),
    /// The loaded, client-side-filterable rows.
    Ready(Vec<RowVm>),
}

/// The three callbacks a picker fires: a row picked, the selection cleared, and "+ New" chosen (with
/// the live query). Bundled so the picker fns stay within the argument budget.
#[derive(Clone, Copy)]
pub struct PickerCallbacks {
    /// Fired when a result row is picked, with its selection.
    pub onpick: Callback<PickerSelection>,
    /// Fired when the collapsed selection is cleared.
    pub onclear: Callback<()>,
    /// Fired when the "+ New …" row is chosen, with the live query text.
    pub onnew: Callback<String>,
}

/// Everything one picker needs to render: its config, the live picker state, the loaded options, the
/// ids to exclude from results (e.g. already-picked partners), and the callbacks it fires.
#[derive(Clone)]
pub struct RecordPicker {
    /// The picker's labels + behaviour flags.
    pub config: PickerConfig,
    /// The live query / open / selection state (owned by the call site so it reseeds cleanly).
    pub state: Signal<PickerState>,
    /// The options to search, loaded once per form.
    pub options: PickerOptions,
    /// Record ids to hide from the results (e.g. already-picked ids).
    pub exclude: Vec<String>,
    /// The callbacks the picker fires on pick / clear / "+ New".
    pub callbacks: PickerCallbacks,
}

/// The read-first view state of a [`draft_picker_field`]: whether the record is being edited, the
/// current selection derived from the draft's `RecordLink` (so it reseeds without stale picker state),
/// and whether that link differs from the committed one (drives the modified tint + reset).
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DraftPickerView {
    /// Whether the record is in edit mode (the picker) rather than view mode (a read box).
    pub editing: bool,
    /// The linked record, derived from the draft's `RecordLink::Existing` (or `None` when unset).
    pub selection: Option<PickerSelection>,
    /// Whether the link differs from its committed value (shows the reset control + tint).
    pub modified: bool,
}

/// Maps a picker resource's read (`use_resource(..).read_unchecked().as_ref()`) to [`PickerOptions`].
#[must_use]
pub fn picker_options(loaded: Option<&Result<Vec<RowVm>, String>>) -> PickerOptions {
    match loaded {
        None => PickerOptions::Loading,
        Some(Err(message)) => PickerOptions::Failed(message.clone()),
        Some(Ok(rows)) => PickerOptions::Ready(rows.clone()),
    }
}

/// A find-or-create record picker (`record-editing.html` §6b): a labelled field showing either a
/// collapsed [`.picker-value`](picker_value) chip once a record is picked, or a search input over the
/// in-flow result list. The list closes only on pick / clear / Esc — never on blur (`WebKitGTK` eats a
/// row click when a blur handler closes the list first).
pub fn record_picker(loc: &Localizer, picker: &RecordPicker) -> Element {
    let selection = picker.state.read().selection.clone();
    rsx! {
        div { class: "field",
            label { r#for: "{picker.config.name}", "{picker.config.label}" }
            if let Some(selection) = selection {
                {picker_value(loc, &selection, false, picker.state, picker.callbacks.onclear, None)}
            } else {
                {picker_search(loc, picker)}
            }
        }
    }
}

/// A read-first record link for the whole-record editor (`event.html` place): in view mode a `.field`
/// read box showing `Title (P0021)`; in edit mode the picker, with the collapsed selection derived
/// from the draft's `RecordLink` (not the picker state) so `use_record_edit` reseeds cleanly, plus a
/// modified tint + reset when the link differs from the committed one.
pub fn draft_picker_field(
    loc: &Localizer,
    picker: &RecordPicker,
    view: &DraftPickerView,
    onreset: Callback<()>,
) -> Element {
    if !view.editing {
        let display = view
            .selection
            .as_ref()
            .map_or_else(|| "—".to_owned(), PickerSelection::display);
        return rsx! {
            div { class: "field",
                label { r#for: "{picker.config.name}", "{picker.config.label}" }
                span { class: "val", "{display}" }
            }
        };
    }
    let reset = Some((onreset, loc.action_reset_field(&picker.config.label)));
    rsx! {
        div { class: "field",
            label { r#for: "{picker.config.name}", "{picker.config.label}" }
            if let Some(selection) = view.selection.clone() {
                {picker_value(loc, &selection, view.modified, picker.state, picker.callbacks.onclear, reset)}
            } else {
                {picker_search(loc, picker)}
            }
        }
    }
}

/// A nested draft card (`design-system.html`): a raised, bordered, offset sub-form for a record being
/// created inline. Its head carries the uppercase title, a "draft" badge, and a discard control (✕);
/// nesting alternates the surface via CSS. The `body` is the record's own fields (which may hold
/// another picker → another card).
pub fn draft_card(title: &str, badge: &str, discard_label: String, ondiscard: Callback<()>, body: Element) -> Element {
    rsx! {
        div { class: "draft-card",
            div { class: "draft-card-head",
                h4 { class: "draft-card-title", "{title}" }
                span { class: "badge draft", "{badge}" }
                IconButton {
                    icon: "✕".to_owned(),
                    label: discard_label.clone(),
                    title: discard_label,
                    onclick: move |_| ondiscard.call(()),
                }
            }
            {body}
        }
    }
}

/// The collapsed selection chip: the title, the mono id, an optional reset control, and the clear (✕)
/// control (labelled `Clear selection`). Clearing empties the picker state and fires `onclear`.
fn picker_value(
    loc: &Localizer,
    selection: &PickerSelection,
    modified: bool,
    mut state: Signal<PickerState>,
    onclear: Callback<()>,
    reset: Option<(Callback<()>, String)>,
) -> Element {
    let class = if modified {
        "picker-value modified"
    } else {
        "picker-value"
    };
    let clear_label = loc.picker_clear();
    rsx! {
        div { class,
            span { class: "val", "{selection.title}" }
            span { class: "row-id", "{selection.human_id}" }
            if let Some((onreset, reset_label)) = reset {
                IconButton {
                    icon: "↺".to_owned(),
                    label: reset_label.clone(),
                    title: reset_label,
                    onclick: move |_| onreset.call(()),
                }
            }
            IconButton {
                icon: "✕".to_owned(),
                label: clear_label.clone(),
                title: clear_label,
                onclick: move |_| {
                    state.write().clear();
                    onclear.call(());
                },
            }
        }
    }
}

/// The search input + in-flow result list. The input opens the list on focus and on each keystroke;
/// Esc closes it (stopping propagation so the record-form Esc does not also cancel the whole edit),
/// and unmodified typing stays local (so `s`/`e` never trigger the record shortcuts).
fn picker_search(loc: &Localizer, picker: &RecordPicker) -> Element {
    let mut state = picker.state;
    let query = state.read().query.clone();
    let open = state.read().open;
    let placeholder = loc.picker_placeholder(&picker.config.entity_label);
    rsx! {
        TextInput {
            id: "{picker.config.name}",
            name: "{picker.config.name}",
            value: "{query}",
            placeholder,
            onfocus: move |_| state.write().open = true,
            oninput: move |event: FormEvent| {
                let mut state = state.write();
                state.query = event.value();
                state.open = true;
            },
            onkeydown_extra: move |event: KeyboardEvent| {
                if event.key() == Key::Escape {
                    event.stop_propagation();
                    state.write().open = false;
                }
            },
        }
        if open {
            {picker_results(loc, picker, &query)}
        }
    }
}

/// The in-flow result list: a reused [`ListRow`] per [`picker_rows`] match (capped at six), a
/// `picker-empty` line when nothing matches, and a trailing "+ New …" row when the picker allows it.
fn picker_results(loc: &Localizer, picker: &RecordPicker, query: &str) -> Element {
    let rows = match &picker.options {
        PickerOptions::Loading => return rsx! {},
        PickerOptions::Failed(message) => {
            return rsx! {
                div { class: "picker-results", role: "listbox",
                    p { class: "picker-empty", "{message}" }
                }
            };
        }
        PickerOptions::Ready(rows) => rows,
    };
    let matched = picker_rows(rows, query, &picker.exclude);
    let empty = matched.is_empty();
    let allow_new = picker.config.allow_new;
    let new_label = if query.is_empty() {
        loc.picker_new(&picker.config.entity_label)
    } else {
        loc.picker_new_query(&picker.config.entity_label, query)
    };
    let onpick = picker.callbacks.onpick;
    let onnew = picker.callbacks.onnew;
    let mut state = picker.state;
    let query = query.to_owned();
    rsx! {
        div { class: "picker-results", role: "listbox",
            for row in matched {
                ListRow {
                    key: "{row.id}",
                    title: row.title.clone(),
                    subtitle: row.subtitle.clone(),
                    id_label: Some(row.id.clone()),
                    avatar: row.avatar.clone(),
                    onclick: {
                        let selection = PickerSelection::from_row(&row);
                        move |_| {
                            state.write().pick(&row);
                            onpick.call(selection.clone());
                        }
                    },
                }
            }
            if empty {
                p { class: "picker-empty", "{loc.picker_empty()}" }
            }
            if allow_new {
                button {
                    class: "picker-new",
                    r#type: "button",
                    onclick: move |_| {
                        onnew.call(query.clone());
                        state.write().open = false;
                    },
                    "{new_label}"
                }
            }
        }
    }
}
