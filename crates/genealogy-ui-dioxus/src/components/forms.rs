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
                onkeydown: move |event| keep_typing_local(&event),
            }
        }
    }
}

/// A free-text date input. Genealogy dates are textual (`12 Apr 1850`, `abt 1850`, `1850–1852`), so
/// this is a text field with a format hint, not a calendar picker.
#[component]
pub fn DatePicker(
    /// The field's already-localized label.
    label: String,
    /// The field's machine name, also used as the element id.
    name: String,
    /// An optional prefilled value.
    #[props(default)]
    value: Option<String>,
    /// An optional, already-localized format hint shown as placeholder.
    #[props(default)]
    placeholder: Option<String>,
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
                onkeydown: move |event| keep_typing_local(&event),
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
) -> Element {
    rsx! {
        div { class: "field",
            label { class: "inline", r#for: "{name}",
                input { r#type: "checkbox", id: "{name}", name: "{name}", checked }
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
