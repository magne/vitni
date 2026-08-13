//! The plugin-UI vocabulary interpreter for Dioxus (ADR 0012, extended by ADR 0022): maps a
//! [`vitni_ui::Panel`] to RSX built on the design-system components.
//!
//! This is the per-framework interpreter ADR 0008 §5 calls for — written once, reused by every
//! plugin panel. A second framework adds its own interpreter over the same `vitni-ui` types.
//! [`FormView`] captures each field's value in a signal (seeded so an untouched form still submits a
//! complete object) and emits a [`PanelAction`] when an action button is activated; the caller runs
//! the submission and, on success, may replace the panel with the returned one (ADR 0022 §2).

use dioxus::prelude::*;
use serde_json::Value;
use vitni_ui::{Action, Field, Form, Panel, Table};

use crate::components::{
    Button, ButtonVariant, Card, Checkbox, DateInput, Input, NumberInput, Select, SelectChoice, Table as TableView,
    Textarea,
};

/// An activated form action (ADR 0022 §2): the button's `action` id and the form's `values` as a
/// JSON object string keyed by field name. The caller submits it to the plugin.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PanelAction {
    /// The activated action's id.
    pub action: String,
    /// The form's field values as a JSON object string.
    pub values: String,
}

/// Renders a plugin-supplied [`Panel`] as native widgets, dispatching on its kind (ADR 0022 §1). A
/// form emits `onaction` when a button is activated; a table is read-only and never fires it.
#[component]
pub fn PanelView(panel: Panel, onaction: EventHandler<PanelAction>) -> Element {
    match panel {
        Panel::Form(form) => rsx! {
            FormView { form, onaction }
        },
        Panel::Table(table) => rsx! {
            PanelTableView { table }
        },
    }
}

/// Renders a plugin [`Form`]: its fields (each writing into a seeded value signal) followed by one
/// button per [`Action`]. Activating a button emits a [`PanelAction`] carrying the collected values.
#[component]
pub fn FormView(form: Form, onaction: EventHandler<PanelAction>) -> Element {
    let values = use_signal(|| seed_values(&form.fields));
    rsx! {
        Card { title: Some(form.title.clone()),
            for field in form.fields.iter() {
                {field_input(field, values)}
            }
            for action in form.actions.iter() {
                {action_button(action, values, onaction)}
            }
        }
    }
}

/// Renders a read-only plugin [`Table`]: localized column headers and literal row cells (ADR 0022 §1).
#[component]
fn PanelTableView(table: Table) -> Element {
    rsx! {
        Card { title: Some(table.title.clone()),
            TableView { caption: table.title.clone(), headers: table.columns.clone(),
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

/// One action button: on click it serializes the current `values` and emits a [`PanelAction`].
fn action_button(action: &Action, values: Signal<Value>, onaction: EventHandler<PanelAction>) -> Element {
    let id = action.id.clone();
    rsx! {
        Button {
            key: "{action.id}",
            label: action.label.clone(),
            variant: ButtonVariant::Primary,
            onclick: move |_| {
                onaction.call(PanelAction {
                    action: id.clone(),
                    values: values.read().to_string(),
                });
            },
        }
    }
}

/// Renders one [`Field`] as a labelled, controlled input that writes its value into `values` by name.
fn field_input(field: &Field, values: Signal<Value>) -> Element {
    match field {
        Field::Text {
            label,
            name,
            placeholder,
        } => {
            let key = name.clone();
            rsx! {
                Input {
                    label: label.clone(),
                    name: name.clone(),
                    placeholder: placeholder.clone(),
                    oninput: move |event: FormEvent| set_value(values, &key, Value::String(event.value())),
                }
            }
        }
        Field::Textarea {
            label,
            name,
            placeholder,
        } => {
            let key = name.clone();
            rsx! {
                Textarea {
                    label: label.clone(),
                    name: name.clone(),
                    placeholder: placeholder.clone(),
                    oninput: move |event: FormEvent| set_value(values, &key, Value::String(event.value())),
                }
            }
        }
        Field::Number { label, name } => {
            let key = name.clone();
            rsx! {
                NumberInput {
                    label: label.clone(),
                    name: name.clone(),
                    oninput: move |event: FormEvent| set_value(values, &key, number_value(&event.value())),
                }
            }
        }
        Field::Date { label, name } => {
            let key = name.clone();
            rsx! {
                DateInput {
                    label: label.clone(),
                    name: name.clone(),
                    oninput: move |event: FormEvent| set_value(values, &key, Value::String(event.value())),
                }
            }
        }
        Field::Checkbox { label, name } => {
            let key = name.clone();
            rsx! {
                Checkbox {
                    label: label.clone(),
                    name: name.clone(),
                    onchange: move |event: FormEvent| set_value(values, &key, Value::Bool(event.checked())),
                }
            }
        }
        Field::Select { label, name, options } => {
            let key = name.clone();
            let seeded = options.first().map(|option| option.value.clone());
            let choices = options
                .iter()
                .map(|option| SelectChoice {
                    value: option.value.clone(),
                    label: option.label.clone(),
                })
                .collect();
            rsx! {
                Select {
                    label: label.clone(),
                    name: name.clone(),
                    value: seeded,
                    options: choices,
                    onchange: move |event: FormEvent| set_value(values, &key, Value::String(event.value())),
                }
            }
        }
    }
}

/// Writes `value` under `name` in the form's value object.
fn set_value(mut values: Signal<Value>, name: &str, value: Value) {
    if let Some(object) = values.write().as_object_mut() {
        object.insert(name.to_owned(), value);
    }
}

/// Seeds the value object for a form's fields (ADR 0022 §2): text-likes and date to `""`, number to
/// `null`, checkbox to `false`, select to its first option's value — so an untouched form submits a
/// complete object.
fn seed_values(fields: &[Field]) -> Value {
    let mut object = serde_json::Map::new();
    for field in fields {
        let (name, value) = seed_field(field);
        object.insert(name, value);
    }
    Value::Object(object)
}

/// The seeded `(name, value)` for one field.
fn seed_field(field: &Field) -> (String, Value) {
    match field {
        Field::Text { name, .. } | Field::Textarea { name, .. } | Field::Date { name, .. } => {
            (name.clone(), Value::String(String::new()))
        }
        Field::Number { name, .. } => (name.clone(), Value::Null),
        Field::Checkbox { name, .. } => (name.clone(), Value::Bool(false)),
        Field::Select { name, options, .. } => {
            let value = options.first().map(|option| option.value.clone()).unwrap_or_default();
            (name.clone(), Value::String(value))
        }
    }
}

/// Encodes a number input's raw text (ADR 0022 §2): empty is `null`, an integer or float parses to a
/// JSON number, and anything else falls back to `null` — never a panic.
fn number_value(raw: &str) -> Value {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return Value::Null;
    }
    if let Ok(integer) = trimmed.parse::<i64>() {
        return Value::Number(integer.into());
    }
    match trimmed.parse::<f64>().ok().and_then(serde_json::Number::from_f64) {
        Some(number) => Value::Number(number),
        None => Value::Null,
    }
}

#[cfg(test)]
mod tests {
    use super::{number_value, seed_values};
    use serde_json::{Value, json};
    use vitni_ui::{Field, SelectOption};

    #[test]
    fn seed_values_seeds_each_field_kind_to_its_empty_default() {
        let fields = vec![
            Field::Text {
                label: "l".to_owned(),
                name: "t".to_owned(),
                placeholder: None,
            },
            Field::Textarea {
                label: "l".to_owned(),
                name: "ta".to_owned(),
                placeholder: None,
            },
            Field::Date {
                label: "l".to_owned(),
                name: "d".to_owned(),
            },
            Field::Number {
                label: "l".to_owned(),
                name: "n".to_owned(),
            },
            Field::Checkbox {
                label: "l".to_owned(),
                name: "c".to_owned(),
            },
            Field::Select {
                label: "l".to_owned(),
                name: "s".to_owned(),
                options: vec![
                    SelectOption {
                        label: "One".to_owned(),
                        value: "one".to_owned(),
                    },
                    SelectOption {
                        label: "Two".to_owned(),
                        value: "two".to_owned(),
                    },
                ],
            },
        ];
        assert_eq!(
            seed_values(&fields),
            json!({ "t": "", "ta": "", "d": "", "n": Value::Null, "c": false, "s": "one" }),
            "text-likes seed empty, number null, checkbox false, select = first option value"
        );
    }

    #[test]
    fn seed_values_select_with_no_options_seeds_empty_string() {
        let fields = vec![Field::Select {
            label: "l".to_owned(),
            name: "s".to_owned(),
            options: vec![],
        }];
        assert_eq!(seed_values(&fields), json!({ "s": "" }));
    }

    #[test]
    fn number_value_maps_empty_to_null_and_parses_ints_and_floats() {
        assert_eq!(number_value(""), Value::Null);
        assert_eq!(number_value("   "), Value::Null);
        assert_eq!(number_value("1900"), json!(1900));
        assert_eq!(number_value("-3"), json!(-3));
        assert_eq!(number_value("2.5"), json!(2.5));
        // Non-numeric text never panics: it falls back to null.
        assert_eq!(number_value("abc"), Value::Null);
    }
}
