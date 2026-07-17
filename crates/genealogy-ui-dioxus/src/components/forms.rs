//! Form widgets: labelled inputs, a select, a checkbox, and a read-only labelled value.
//!
//! Controlled: each takes its `value` as a prop and forwards edits via an optional change handler,
//! so the call site owns the state (the Person editing slice, PR4, wires them to field signals).
//! Omitting the handler leaves a widget display-only, as the detail views use it.
//!
//! Every text/select control is rendered through the [`TextInput`]/[`SelectInput`] behavior core, so
//! the global-shortcut typing guard is wired once (see `text_input.rs`); only the [`Checkbox`]'s
//! non-typing `type="checkbox"` input is a raw element here.

use dioxus::prelude::*;

use crate::components::select_input::{SelectChoice, SelectInput};
use crate::components::text_input::{TextInput, TextInputKind};

/// A single-line text input with a label.
#[component]
pub fn Input(
    /// The field's already-localized label.
    label: String,
    /// The field's machine name, also used as the element id for label association.
    name: String,
    /// An optional prefilled value.
    #[props(default)]
    value: Option<String>,
    /// Optional placeholder text.
    #[props(default)]
    placeholder: Option<String>,
    /// Fired on each input event with the form event (omit for a display-only field).
    #[props(default)]
    oninput: Option<EventHandler<FormEvent>>,
) -> Element {
    rsx! {
        div { class: "field",
            label { r#for: "{name}", "{label}" }
            TextInput { id: "{name}", name: name.clone(), value, placeholder, oninput }
        }
    }
}

/// A numeric input with a label.
#[component]
pub fn NumberInput(
    /// The field's already-localized label.
    label: String,
    /// The field's machine name, also used as the element id.
    name: String,
    /// An optional prefilled value.
    #[props(default)]
    value: Option<String>,
    /// Fired on each input event with the form event (omit for a display-only field).
    #[props(default)]
    oninput: Option<EventHandler<FormEvent>>,
) -> Element {
    rsx! {
        div { class: "field",
            label { r#for: "{name}", "{label}" }
            TextInput { id: "{name}", name: name.clone(), value, kind: TextInputKind::Number, oninput }
        }
    }
}

/// A multi-line text input with a label (ADR 0022 `textarea` field kind). Controlled like [`Input`].
#[component]
pub fn Textarea(
    /// The field's already-localized label.
    label: String,
    /// The field's machine name, also used as the element id for label association.
    name: String,
    /// An optional prefilled value.
    #[props(default)]
    value: Option<String>,
    /// Optional placeholder text.
    #[props(default)]
    placeholder: Option<String>,
    /// Fired on each input event with the form event (omit for a display-only field).
    #[props(default)]
    oninput: Option<EventHandler<FormEvent>>,
) -> Element {
    rsx! {
        div { class: "field",
            label { r#for: "{name}", "{label}" }
            TextInput { id: "{name}", name: name.clone(), value, placeholder, multiline: true, oninput }
        }
    }
}

/// A plain date input with a label (ADR 0022 `date` field kind). The wire value is an ISO-8601
/// `YYYY-MM-DD` string; this is the browser date control, not the app's structured date cluster.
#[component]
pub fn DateInput(
    /// The field's already-localized label.
    label: String,
    /// The field's machine name, also used as the element id for label association.
    name: String,
    /// An optional prefilled value (`YYYY-MM-DD`).
    #[props(default)]
    value: Option<String>,
    /// Fired on each input event with the form event (omit for a display-only field).
    #[props(default)]
    oninput: Option<EventHandler<FormEvent>>,
) -> Element {
    rsx! {
        div { class: "field",
            label { r#for: "{name}", "{label}" }
            TextInput { id: "{name}", name: name.clone(), value, kind: TextInputKind::Date, oninput }
        }
    }
}

/// The structured-date control cluster (`event.html` edit specimen): a modifier select, one or two
/// free-typed date inputs, and quality + calendar selects. A dumb, controlled component — every value
/// and option is an already-localized string/[`SelectChoice`] and every edit is forwarded through a
/// per-control handler, so the call site's draft owns the state.
///
/// The end (second) date input renders only for a range/span (`show_end`); the date inputs are hidden
/// for a text-only date (`show_date_inputs`), whose value comes from the separate Original-text field.
#[component]
pub fn DatePicker(
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
    /// The modifier options, in display order.
    modifier_options: Vec<SelectChoice>,
    /// The selected modifier option value.
    modifier_value: String,
    /// The quality options, in display order.
    quality_options: Vec<SelectChoice>,
    /// The selected quality option value.
    quality_value: String,
    /// The calendar options, in display order.
    calendar_options: Vec<SelectChoice>,
    /// The selected calendar option value.
    calendar_value: String,
    /// The (start) date input's current text.
    start_value: String,
    /// The end date input's current text (range/span only).
    end_value: String,
    /// Whether to render the end date input (range/span).
    show_end: bool,
    /// Whether to render the date inputs at all (hidden for a text-only date).
    show_date_inputs: bool,
    /// Whether the (start) date input is invalid (drives `aria-invalid`).
    #[props(default)]
    invalid: bool,
    /// Fired when the modifier changes, with the chosen option value.
    onmodifier: EventHandler<String>,
    /// Fired on each (start) date input, with the new text.
    onstart: EventHandler<String>,
    /// Fired on each end date input, with the new text.
    onend: EventHandler<String>,
    /// Fired when the quality changes, with the chosen option value.
    onquality: EventHandler<String>,
    /// Fired when the calendar changes, with the chosen option value.
    oncalendar: EventHandler<String>,
) -> Element {
    rsx! {
        div { class: "fact-row",
            SelectInput {
                style: "width:auto",
                aria_label: "{modifier_label}",
                selected: modifier_value,
                options: modifier_options,
                onchange: move |event: FormEvent| onmodifier.call(event.value()),
            }
            if show_date_inputs {
                TextInput {
                    style: "width:130px",
                    aria_label: "{date_label}",
                    invalid,
                    value: "{start_value}",
                    oninput: move |event: FormEvent| onstart.call(event.value()),
                }
                if show_end {
                    TextInput {
                        style: "width:130px",
                        aria_label: "{end_label}",
                        value: "{end_value}",
                        oninput: move |event: FormEvent| onend.call(event.value()),
                    }
                }
            }
            SelectInput {
                style: "width:auto",
                aria_label: "{quality_label}",
                selected: quality_value,
                options: quality_options,
                onchange: move |event: FormEvent| onquality.call(event.value()),
            }
            SelectInput {
                style: "width:auto",
                aria_label: "{calendar_label}",
                selected: calendar_value,
                options: calendar_options,
                onchange: move |event: FormEvent| oncalendar.call(event.value()),
            }
        }
    }
}

/// A checkbox with an inline label.
#[component]
pub fn Checkbox(
    /// The checkbox's already-localized label.
    label: String,
    /// The field's machine name, also used as the element id.
    name: String,
    /// Whether the box is checked.
    #[props(default)]
    checked: bool,
    /// Fired on toggle with the form event (omit for a display-only checkbox).
    #[props(default)]
    onchange: Option<EventHandler<FormEvent>>,
) -> Element {
    rsx! {
        div { class: "field",
            label { class: "inline", r#for: "{name}",
                input {
                    r#type: "checkbox",
                    id: "{name}",
                    name: "{name}",
                    checked,
                    onchange: move |event| {
                        if let Some(onchange) = &onchange {
                            onchange.call(event);
                        }
                    },
                }
                span { "{label}" }
            }
        }
    }
}

/// A single-choice dropdown with a label.
#[component]
pub fn Select(
    /// The field's already-localized label.
    label: String,
    /// The field's machine name, also used as the element id.
    name: String,
    /// The currently selected value, if any.
    #[props(default)]
    value: Option<String>,
    /// The selectable options, in display order.
    options: Vec<SelectChoice>,
    /// Fired on change with the form event (omit for a display-only select).
    #[props(default)]
    onchange: Option<EventHandler<FormEvent>>,
) -> Element {
    rsx! {
        div { class: "field",
            label { r#for: "{name}", "{label}" }
            SelectInput {
                id: "{name}",
                name: name.clone(),
                selected: value.unwrap_or_default(),
                options,
                onchange,
            }
        }
    }
}

/// A read-only labelled value (the detail-view counterpart to an input).
#[component]
pub fn LabeledValue(
    /// The already-localized field label.
    label: String,
    /// The already-localized value text.
    value: String,
) -> Element {
    rsx! {
        div { class: "field",
            label { "{label}" }
            div { class: "val", "{value}" }
        }
    }
}
