//! The plugin-UI vocabulary interpreter for Dioxus (ADR 0012, extended by ADR 0022): maps a
//! [`genealogy_ui::Panel`] to RSX built on the design-system components.
//!
//! This is the per-framework interpreter ADR 0008 §5 calls for — written once, reused by every
//! plugin panel. A second framework adds its own interpreter over the same `genealogy-ui` types.
//! Action buttons are inert here; value capture and submission are wired by the caller (ADR 0022 §2).

use dioxus::prelude::*;
use genealogy_ui::{Field, Form, Panel, Table};

use crate::components::{
    Button, ButtonVariant, Card, Checkbox, DateInput, Input, NumberInput, Select, SelectChoice, Table as TableView,
    Textarea,
};

/// Renders a plugin-supplied [`Panel`] as native widgets, dispatching on its kind (ADR 0022 §1).
#[component]
pub fn PanelView(panel: Panel) -> Element {
    match panel {
        Panel::Form(form) => rsx! {
            FormView { form }
        },
        Panel::Table(table) => rsx! {
            PanelTableView { table }
        },
    }
}

/// Renders a plugin [`Form`]: its fields followed by one button per [`genealogy_ui::Action`]. The
/// buttons are inert here — the caller wires value capture and submission (ADR 0022 §2).
#[component]
pub fn FormView(form: Form) -> Element {
    rsx! {
        Card { title: Some(form.title.clone()),
            for field in form.fields.iter() {
                FieldView { field: field.clone() }
            }
            for action in form.actions.iter() {
                Button {
                    key: "{action.id}",
                    label: action.label.clone(),
                    variant: ButtonVariant::Primary,
                    onclick: move |_| {},
                }
            }
        }
    }
}

/// Renders a read-only plugin [`Table`]: localized column headers and literal row cells (ADR 0022 §1).
#[component]
fn PanelTableView(table: Table) -> Element {
    rsx! {
        Card { title: Some(table.title.clone()),
            TableView { headers: table.columns.clone(),
                for row in table.rows.iter() {
                    tr {
                        for cell in row.iter() {
                            td { "{cell}" }
                        }
                    }
                }
            }
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
        Field::Textarea {
            label,
            name,
            placeholder,
        } => rsx! {
            Textarea { label, name, placeholder }
        },
        Field::Number { label, name } => rsx! {
            NumberInput { label, name }
        },
        Field::Date { label, name } => rsx! {
            DateInput { label, name }
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
