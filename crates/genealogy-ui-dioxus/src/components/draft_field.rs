//! No-reflow record fields (`record-editing.html` §3/§4): one control that renders as a read box in
//! view mode and as an input (with per-field reset) in edit mode, so toggling a field between modes
//! moves no text. Generalized from the tag record editor's `.field-with-revert`.
//!
//! Controlled: each takes its current `value`, the committed `original`, and forwards edits/reset via
//! event handlers, so the call site's draft owns the state. A field whose `value` differs from its
//! `original` is *modified* — tinted and showing a reset control; a `locked` field renders a disabled
//! input rather than jumping to read text.

use dioxus::prelude::*;
use genealogy_ui::{DATE_CALENDARS, DATE_QUALITIES, DateDraft, DateModifierKind, Localizer};

use crate::components::{DatePicker, IconButton, SelectChoice, SelectInput, TextInput};

/// The modifier-select options for the given kind (index-valued, localized labels): the nine offered
/// options, plus Interpreted when the current kind is Interpreted (a seeded value).
#[must_use]
pub fn date_modifier_options(loc: &Localizer, kind: DateModifierKind) -> Vec<SelectChoice> {
    kind.choices_for()
        .iter()
        .enumerate()
        .map(|(index, kind)| SelectChoice {
            value: index.to_string(),
            label: loc.date_modifier_choice_label(*kind),
        })
        .collect()
}

/// The quality-select options (index-valued, localized labels).
#[must_use]
pub fn date_quality_options(loc: &Localizer) -> Vec<SelectChoice> {
    DATE_QUALITIES
        .iter()
        .enumerate()
        .map(|(index, quality)| SelectChoice {
            value: index.to_string(),
            label: loc.date_quality_choice_label(*quality),
        })
        .collect()
}

/// The calendar-select options (index-valued, localized labels).
#[must_use]
pub fn date_calendar_options(loc: &Localizer) -> Vec<SelectChoice> {
    DATE_CALENDARS
        .iter()
        .enumerate()
        .map(|(index, calendar)| SelectChoice {
            value: index.to_string(),
            label: loc.calendar_label(*calendar),
        })
        .collect()
}

/// The localized validation message for a date field, or `None` when it parses: a text-only date needs
/// its original text; any other kind needs a valid date.
#[must_use]
pub fn date_field_error(loc: &Localizer, date: &DateDraft) -> Option<String> {
    if !date.is_invalid() {
        return None;
    }
    Some(if date.kind == DateModifierKind::TextOnly {
        loc.date_text_required_error()
    } else {
        loc.date_invalid_error()
    })
}

/// The whole-record date field wired to a draft: builds the localized options/labels and renders the
/// [`DraftDate`] editor, forwarding an updated [`DateDraft`] through `onchange` and the revert through
/// `onreset`. The three record screens share this so the ~20-prop wiring lives once.
pub fn date_draft_field(
    loc: &Localizer,
    name: &str,
    editing: bool,
    value: DateDraft,
    original: DateDraft,
    onchange: Callback<DateDraft>,
    onreset: Callback<()>,
) -> Element {
    let error = date_field_error(loc, &value);
    rsx! {
        DraftDate {
            label: loc.field_label("date"),
            name: name.to_owned(),
            editing,
            modifier_options: date_modifier_options(loc, value.kind),
            quality_options: date_quality_options(loc),
            calendar_options: date_calendar_options(loc),
            value,
            original,
            modifier_label: loc.date_modifier_label(),
            date_label: loc.field_label("date"),
            quality_label: loc.date_quality_label(),
            calendar_label: loc.date_calendar_label(),
            end_label: loc.date_end_label(),
            original_label: loc.field_original_text(),
            original_hint: loc.date_original_text_hint(),
            reset_label: loc.action_reset_field(&loc.field_label("date")),
            error,
            onchange: move |value: DateDraft| onchange.call(value),
            onreset: move |()| onreset.call(()),
        }
    }
}

/// A single-line text record field with per-field reset.
#[component]
pub fn DraftText(
    /// The field's already-localized label.
    label: String,
    /// The field's machine name / element id.
    name: String,
    /// Whether the record is in edit mode (inputs) or view mode (read box).
    editing: bool,
    /// The draft's current value.
    value: String,
    /// The committed value the field reverts to.
    original: String,
    /// The already-localized accessible name for the reset control.
    reset_label: String,
    /// An optional already-localized validation message (also drives `aria-invalid`).
    #[props(default)]
    error: Option<String>,
    /// An optional already-localized hint shown under the input in edit mode (e.g. "empty ⇒ generate").
    #[props(default)]
    hint: Option<String>,
    /// Whether the field is locked (§3): a disabled input in edit mode.
    #[props(default)]
    locked: bool,
    /// Whether to render the value in the monospace face (e.g. a human id or hex).
    #[props(default)]
    mono: bool,
    /// Whether to render a multi-line `textarea` rather than a single-line input (e.g. note content).
    #[props(default)]
    multiline: bool,
    /// Fired on each input with the new value.
    oninput: EventHandler<String>,
    /// Fired when the reset control is pressed.
    onreset: EventHandler<()>,
) -> Element {
    let modified = value != original;
    let mono_style = if mono { "font-family:var(--font-mono)" } else { "" };
    if !editing {
        return rsx! {
            div { class: "field",
                label { r#for: "{name}", "{label}" }
                span { class: "val", style: "{mono_style}", "{value}" }
            }
        };
    }
    let mut input_class = String::from("in");
    if modified {
        input_class.push_str(" modified");
    }
    let rows = if multiline { Some("5".to_owned()) } else { None };
    rsx! {
        div { class: "field",
            label { r#for: "{name}", "{label}" }
            div { class: "field-with-revert",
                TextInput {
                    id: "{name}",
                    name: "{name}",
                    class: input_class,
                    style: "{mono_style}",
                    multiline,
                    rows,
                    value: Some(value.clone()),
                    disabled: locked,
                    invalid: error.is_some(),
                    oninput: move |event: FormEvent| oninput.call(event.value()),
                }
                if modified && !locked {
                    IconButton {
                        icon: "↺".to_owned(),
                        label: reset_label.clone(),
                        title: reset_label.clone(),
                        onclick: move |_| onreset.call(()),
                    }
                }
            }
            if let Some(message) = error {
                div { class: "field-error", "{message}" }
            }
            if let Some(hint) = hint {
                div { class: "field-hint", "{hint}" }
            }
        }
    }
}

/// A single-choice record field with per-field reset. View mode shows the selected option's label.
#[component]
pub fn DraftSelect(
    /// The field's already-localized label.
    label: String,
    /// The field's machine name / element id.
    name: String,
    /// Whether the record is in edit mode (a select) or view mode (read box).
    editing: bool,
    /// The currently-selected option value.
    value: String,
    /// The committed value the field reverts to.
    original: String,
    /// The already-localized accessible name for the reset control.
    reset_label: String,
    /// The selectable options, in display order.
    options: Vec<crate::components::SelectChoice>,
    /// Whether the field is locked (§3): a disabled select in edit mode.
    #[props(default)]
    locked: bool,
    /// Fired on change with the new value.
    onchange: EventHandler<String>,
    /// Fired when the reset control is pressed.
    onreset: EventHandler<()>,
) -> Element {
    let modified = value != original;
    let selected_label = options
        .iter()
        .find(|option| option.value == value)
        .map(|option| option.label.clone())
        .unwrap_or_default();
    if !editing {
        return rsx! {
            div { class: "field",
                label { r#for: "{name}", "{label}" }
                span { class: "val", "{selected_label}" }
            }
        };
    }
    let select_class = if modified { "in modified" } else { "in" };
    rsx! {
        div { class: "field",
            label { r#for: "{name}", "{label}" }
            div { class: "field-with-revert",
                SelectInput {
                    id: "{name}",
                    name: "{name}",
                    class: select_class,
                    selected: value.clone(),
                    disabled: locked,
                    options,
                    onchange: move |event: FormEvent| onchange.call(event.value()),
                }
                if modified && !locked {
                    IconButton {
                        icon: "↺".to_owned(),
                        label: reset_label.clone(),
                        title: reset_label.clone(),
                        onclick: move |_| onreset.call(()),
                    }
                }
            }
        }
    }
}

/// A structured-date record field (`event.html` edit specimen): the [`DatePicker`] control cluster
/// plus the always-retained Original-text field, with a per-field reset. View mode shows the
/// localized read box.
///
/// Controlled at the whole-[`DateDraft`] granularity: each sub-control edit produces an updated draft
/// through `onchange`, so the call site stores one `DateDraft`. The option lists carry index values
/// (`0`, `1`, …) matching [`DateModifierKind::choices_for`] / [`DATE_QUALITIES`] / [`DATE_CALENDARS`].
#[component]
pub fn DraftDate(
    /// The field's already-localized label.
    label: String,
    /// The field's machine name / element id.
    name: String,
    /// Whether the record is in edit mode (the control cluster) or view mode (read box).
    editing: bool,
    /// The draft's current date.
    value: DateDraft,
    /// The committed date the field reverts to.
    original: DateDraft,
    /// The modifier options (index-valued, already-localized labels).
    modifier_options: Vec<SelectChoice>,
    /// The quality options (index-valued, already-localized labels).
    quality_options: Vec<SelectChoice>,
    /// The calendar options (index-valued, already-localized labels).
    calendar_options: Vec<SelectChoice>,
    /// The accessible name for the modifier select.
    modifier_label: String,
    /// The accessible name for the (start) date input.
    date_label: String,
    /// The accessible name for the quality select.
    quality_label: String,
    /// The accessible name for the calendar select.
    calendar_label: String,
    /// The accessible name for the end date input.
    end_label: String,
    /// The already-localized label for the Original-text field.
    original_label: String,
    /// The already-localized hint under the Original-text field.
    original_hint: String,
    /// The already-localized accessible name for the reset control.
    reset_label: String,
    /// An optional already-localized validation message (also drives `aria-invalid`).
    #[props(default)]
    error: Option<String>,
    /// Fired with the updated draft on any sub-control edit.
    onchange: EventHandler<DateDraft>,
    /// Fired when the reset control is pressed.
    onreset: EventHandler<()>,
) -> Element {
    if !editing {
        return rsx! {
            div { class: "field",
                label { r#for: "{name}", "{label}" }
                span { class: "val", "{value.display}" }
            }
        };
    }
    let modified = value != original;
    let choices = value.kind.choices_for();
    let modifier_value = choices
        .iter()
        .position(|kind| *kind == value.kind)
        .unwrap_or(0)
        .to_string();
    let quality_value = DATE_QUALITIES
        .iter()
        .position(|quality| *quality == value.quality)
        .unwrap_or(0)
        .to_string();
    let calendar_value = DATE_CALENDARS
        .iter()
        .position(|calendar| *calendar == value.calendar)
        .unwrap_or(0)
        .to_string();
    let revert_class = if modified {
        "field-with-revert modified"
    } else {
        "field-with-revert"
    };
    let error_present = error.is_some();
    rsx! {
        div { class: "field",
            label { r#for: "{name}", "{label}" }
            div { class: "{revert_class}",
                DatePicker {
                    modifier_label,
                    date_label,
                    quality_label,
                    calendar_label,
                    end_label,
                    modifier_options,
                    modifier_value,
                    quality_options,
                    quality_value,
                    calendar_options,
                    calendar_value,
                    start_value: value.start.clone(),
                    end_value: value.end.clone(),
                    show_end: value.kind.uses_end(),
                    show_date_inputs: value.kind != DateModifierKind::TextOnly,
                    invalid: error_present,
                    onmodifier: {
                        let value = value.clone();
                        let choices = choices.clone();
                        move |index: String| {
                            if let Some(kind) = index.parse::<usize>().ok().and_then(|index| choices.get(index)) {
                                let mut draft = value.clone();
                                draft.kind = *kind;
                                onchange.call(draft);
                            }
                        }
                    },
                    onstart: {
                        let value = value.clone();
                        move |text: String| {
                            let mut draft = value.clone();
                            draft.start = text;
                            onchange.call(draft);
                        }
                    },
                    onend: {
                        let value = value.clone();
                        move |text: String| {
                            let mut draft = value.clone();
                            draft.end = text;
                            onchange.call(draft);
                        }
                    },
                    onquality: {
                        let value = value.clone();
                        move |index: String| {
                            if let Some(quality) = index.parse::<usize>().ok().and_then(|index| DATE_QUALITIES.get(index)) {
                                let mut draft = value.clone();
                                draft.quality = *quality;
                                onchange.call(draft);
                            }
                        }
                    },
                    oncalendar: {
                        let value = value.clone();
                        move |index: String| {
                            if let Some(calendar) = index.parse::<usize>().ok().and_then(|index| DATE_CALENDARS.get(index)) {
                                let mut draft = value.clone();
                                draft.calendar = *calendar;
                                onchange.call(draft);
                            }
                        }
                    },
                }
                if modified {
                    IconButton {
                        icon: "↺".to_owned(),
                        label: reset_label.clone(),
                        title: reset_label.clone(),
                        onclick: move |_| onreset.call(()),
                    }
                }
            }
            TextInput {
                id: "{name}-original",
                aria_label: "{original_label}",
                value: Some(value.original_text.clone()),
                oninput: {
                    let value = value.clone();
                    move |event: FormEvent| {
                        let mut draft = value.clone();
                        draft.original_text = event.value();
                        onchange.call(draft);
                    }
                },
            }
            div { class: "field-hint", "{original_hint}" }
            if let Some(message) = error {
                div { class: "field-error", "{message}" }
            }
        }
    }
}
