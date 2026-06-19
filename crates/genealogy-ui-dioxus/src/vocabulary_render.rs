//! The plugin-UI vocabulary interpreter for Dioxus (ADR 0012): maps a [`genealogy_ui::Form`] to RSX.
//!
//! This is the per-framework interpreter ADR 0008 §5 calls for — written once, reused by every
//! plugin form. A second framework adds its own interpreter over the same `genealogy-ui` types.

use dioxus::prelude::*;
use genealogy_ui::{Field, Form};

/// Renders a plugin-supplied [`Form`] as native widgets.
#[component]
pub fn FormView(form: Form) -> Element {
    rsx! {
        section { class: "plugin-form",
            h2 { "{form.title}" }
            for field in form.fields.iter() {
                FieldView { field: field.clone() }
            }
            button { class: "submit", "{form.submit}" }
        }
    }
}

/// Renders one [`Field`] as a labelled input.
#[component]
fn FieldView(field: Field) -> Element {
    match field {
        Field::Text {
            label,
            name,
            placeholder,
        } => rsx! {
            label { class: "field",
                span { "{label}" }
                input { r#type: "text", name: "{name}", placeholder: placeholder.unwrap_or_default() }
            }
        },
        Field::Number { label, name } => rsx! {
            label { class: "field",
                span { "{label}" }
                input { r#type: "number", name: "{name}" }
            }
        },
        Field::Checkbox { label, name } => rsx! {
            label { class: "field checkbox",
                input { r#type: "checkbox", name: "{name}" }
                span { "{label}" }
            }
        },
        Field::Select { label, name, options } => rsx! {
            label { class: "field",
                span { "{label}" }
                select { name: "{name}",
                    for option in options.iter() {
                        option { value: "{option.value}", "{option.label}" }
                    }
                }
            }
        },
    }
}
