//! The plugin-UI vocabulary interpreter for Dioxus (ADR 0012): maps a [`genealogy_ui::Form`] to RSX
//! built on the design-system form components.
//!
//! This is the per-framework interpreter ADR 0008 §5 calls for — written once, reused by every
//! plugin form. A second framework adds its own interpreter over the same `genealogy-ui` types.
//! Submission/actions are out of scope until the vocabulary expansion (PR17), so the submit button
//! is inert here.

use dioxus::prelude::*;
use genealogy_ui::{Field, Form};

use crate::components::{Button, ButtonVariant, Card, Checkbox, Input, NumberInput, Select, SelectChoice};

/// Renders a plugin-supplied [`Form`] as native widgets.
#[component]
pub fn FormView(form: Form) -> Element {
    rsx! {
        Card { title: Some(form.title.clone()),
            for field in form.fields.iter() {
                FieldView { field: field.clone() }
            }
            Button { label: form.submit.clone(), variant: ButtonVariant::Primary, onclick: move |_| {} }
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
            Input { label, name, placeholder }
        },
        Field::Number { label, name } => rsx! {
            NumberInput { label, name }
        },
        Field::Checkbox { label, name } => rsx! {
            Checkbox { label, name }
        },
        Field::Select { label, name, options } => {
            let options = options
                .into_iter()
                .map(|option| SelectChoice {
                    value: option.value,
                    label: option.label,
                })
                .collect();
            rsx! {
                Select { label, name, options }
            }
        }
    }
}
