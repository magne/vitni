//! Form widgets: labelled inputs, a select, a checkbox, and a read-only labelled value.
//!
//! Controlled: each takes its `value` as a prop and forwards edits via an optional change handler,
//! so the call site owns the state (the Person editing slice, PR4, wires them to field signals).
//! Omitting the handler leaves a widget display-only, as the detail views use it.

use crate::shell::focus_trap::keep_typing_local;
use dioxus::prelude::*;

/// One option in a [`Select`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectChoice {
    /// The value submitted when this option is chosen.
    pub value: String,
    /// The visible, already-localized label.
    pub label: String,
}

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
            input {
                class: "in",
                r#type: "text",
                id: "{name}",
                name: "{name}",
                value,
                placeholder,
                oninput: move |event| {
                    if let Some(oninput) = &oninput {
                        oninput.call(event);
                    }
                },
                onkeydown: move |event| keep_typing_local(&event),
            }
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
            input {
                class: "in",
                r#type: "number",
                id: "{name}",
                name: "{name}",
                value,
                oninput: move |event| {
                    if let Some(oninput) = &oninput {
                        oninput.call(event);
                    }
                },
                onkeydown: move |event| keep_typing_local(&event),
            }
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
            textarea {
                class: "in",
                id: "{name}",
                name: "{name}",
                value,
                placeholder,
                oninput: move |event| {
                    if let Some(oninput) = &oninput {
                        oninput.call(event);
                    }
                },
                onkeydown: move |event| keep_typing_local(&event),
            }
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
            input {
                class: "in",
                r#type: "date",
                id: "{name}",
                name: "{name}",
                value,
                oninput: move |event| {
                    if let Some(oninput) = &oninput {
                        oninput.call(event);
                    }
                },
                onkeydown: move |event| keep_typing_local(&event),
            }
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
    let aria_invalid = if invalid { "true" } else { "false" };
    let start_class = if invalid { "in invalid" } else { "in" };
    rsx! {
        div { class: "fact-row",
            select {
                class: "in",
                style: "width:auto",
                aria_label: "{modifier_label}",
                onchange: move |event| onmodifier.call(event.value()),
                onkeydown: move |event| keep_typing_local(&event),
                for option in modifier_options.iter() {
                    option { value: "{option.value}", selected: option.value == modifier_value, "{option.label}" }
                }
            }
            if show_date_inputs {
                input {
                    class: "{start_class}",
                    style: "width:130px",
                    r#type: "text",
                    aria_label: "{date_label}",
                    aria_invalid,
                    value: "{start_value}",
                    oninput: move |event| onstart.call(event.value()),
                    onkeydown: move |event| keep_typing_local(&event),
                }
                if show_end {
                    input {
                        class: "in",
                        style: "width:130px",
                        r#type: "text",
                        aria_label: "{end_label}",
                        value: "{end_value}",
                        oninput: move |event| onend.call(event.value()),
                        onkeydown: move |event| keep_typing_local(&event),
                    }
                }
            }
            select {
                class: "in",
                style: "width:auto",
                aria_label: "{quality_label}",
                onchange: move |event| onquality.call(event.value()),
                onkeydown: move |event| keep_typing_local(&event),
                for option in quality_options.iter() {
                    option { value: "{option.value}", selected: option.value == quality_value, "{option.label}" }
                }
            }
            select {
                class: "in",
                style: "width:auto",
                aria_label: "{calendar_label}",
                onchange: move |event| oncalendar.call(event.value()),
                onkeydown: move |event| keep_typing_local(&event),
                for option in calendar_options.iter() {
                    option { value: "{option.value}", selected: option.value == calendar_value, "{option.label}" }
                }
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
            select {
                class: "in",
                id: "{name}",
                name: "{name}",
                onchange: move |event| {
                    if let Some(onchange) = &onchange {
                        onchange.call(event);
                    }
                },
                onkeydown: move |event| keep_typing_local(&event),
                for option in options.iter() {
                    option {
                        value: "{option.value}",
                        selected: value.as_deref() == Some(option.value.as_str()),
                        "{option.label}"
                    }
                }
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
