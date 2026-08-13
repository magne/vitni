//! The plugin-UI vocabulary (ADR 0012, extended by ADR 0022): a small, serializable panel a plugin
//! emits and a framework renderer interprets.
//!
//! A plugin returns the panel as a JSON document (it is a sandboxed component that cannot link these
//! types — the contract is the documented JSON shape, not this crate). [`parse`] validates that JSON
//! into a [`Panel`]. The encoding follows the project's **internally-tagged** convention (a `kind`
//! discriminator on the panel and on each field), matching the event encoding (ADR 0004 §4), and is
//! **additive**: new field kinds and panel kinds are appended without breaking older renderers.
//!
//! A [`Panel`] is either a [`Form`] (a title, typed fields, and one or more [`Action`] buttons) or a
//! read-only [`Table`]. Activating an action submits the form's field values back to the plugin,
//! which returns a [`SubmitResult`] parsed by [`parse_submit_result`] (ADR 0022 §2).

use serde::{Deserialize, Serialize};

/// A plugin-contributed screen: either an editable [`Form`] or a read-only [`Table`] (ADR 0022 §1).
/// The `kind` tag selects the variant (internally-tagged JSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Panel {
    /// An editable form with action buttons.
    Form(Form),
    /// A read-only data table.
    Table(Table),
}

/// A single-screen form: a title, an ordered list of typed fields, and one or more action buttons.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Form {
    /// The form's heading.
    pub title: String,
    /// The fields, rendered top to bottom in order.
    pub fields: Vec<Field>,
    /// The action buttons offered below the fields (ADR 0022 §1); at least one.
    pub actions: Vec<Action>,
}

/// One action button on a [`Form`]. Activating it submits the field values under this action's id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Action {
    /// The machine id the submission carries so the plugin knows which button was activated. Never
    /// localized.
    pub id: String,
    /// The button's user-facing label — a Fluent message id (ADR 0022 §5).
    pub label: String,
}

/// A read-only data table (ADR 0022 §1). `title` and `columns` are Fluent message ids; `rows` hold
/// literal data cells that are never resolved.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Table {
    /// The table's heading — a Fluent message id.
    pub title: String,
    /// The column headers, in display order — each a Fluent message id.
    pub columns: Vec<String>,
    /// The body rows; each inner vector is one row's literal cells (never resolved).
    pub rows: Vec<Vec<String>>,
}

/// One input field. The `kind` tag selects the variant (internally-tagged JSON).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum Field {
    /// A single-line text input.
    Text {
        /// The field's user-facing label.
        label: String,
        /// The machine name a submission keys this field by.
        name: String,
        /// Optional placeholder text shown when empty.
        #[serde(default)]
        placeholder: Option<String>,
    },
    /// A multi-line text input (ADR 0022 §1).
    Textarea {
        /// The field's user-facing label.
        label: String,
        /// The machine name a submission keys this field by.
        name: String,
        /// Optional placeholder text shown when empty.
        #[serde(default)]
        placeholder: Option<String>,
    },
    /// A numeric input.
    Number {
        /// The field's user-facing label.
        label: String,
        /// The machine name a submission keys this field by.
        name: String,
    },
    /// A plain date input (ADR 0022 §1): the wire value is an ISO-8601 `YYYY-MM-DD` string or `""`.
    /// This is deliberately not the app's structured `GenealogicalDate` picker.
    Date {
        /// The field's user-facing label.
        label: String,
        /// The machine name a submission keys this field by.
        name: String,
    },
    /// A boolean checkbox.
    Checkbox {
        /// The field's user-facing label.
        label: String,
        /// The machine name a submission keys this field by.
        name: String,
    },
    /// A single-choice dropdown.
    Select {
        /// The field's user-facing label.
        label: String,
        /// The machine name a submission keys this field by.
        name: String,
        /// The selectable options, in display order.
        options: Vec<SelectOption>,
    },
}

/// One option of a [`Field::Select`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SelectOption {
    /// The user-facing label.
    pub label: String,
    /// The value a submission carries when this option is chosen.
    pub value: String,
}

/// The result a plugin returns from `handle-action` (ADR 0022 §2). The `kind` tag selects the
/// variant (internally-tagged JSON). Validation feedback rides `failure`; a technical failure is the
/// WIT `Err(string)` channel instead.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub enum SubmitResult {
    /// The action succeeded.
    Success {
        /// An optional confirmation message — a Fluent message id (ADR 0022 §5).
        #[serde(default)]
        message: Option<String>,
        /// An optional panel to display in place of the current one (e.g. a preview table).
        #[serde(default)]
        panel: Option<Panel>,
    },
    /// The action failed validation; `message` is a Fluent message id to show the user.
    Failure {
        /// The failure message — a Fluent message id (ADR 0022 §5).
        message: String,
    },
}

/// A failure to parse a plugin's panel JSON against the vocabulary schema (ADR 0012, ADR 0022).
#[derive(Debug, thiserror::Error)]
pub enum VocabularyError {
    /// The JSON did not match the [`Panel`] (or [`SubmitResult`]) schema.
    #[error("malformed plugin panel: {0}")]
    Malformed(String),
}

/// Parses a plugin-supplied JSON document into a [`Panel`].
///
/// # Errors
///
/// [`VocabularyError::Malformed`] if `json` is not valid JSON or does not match the schema.
pub fn parse(json: &str) -> Result<Panel, VocabularyError> {
    serde_json::from_str(json).map_err(|error| VocabularyError::Malformed(error.to_string()))
}

/// Parses a plugin's `handle-action` `Ok` payload into a [`SubmitResult`] (ADR 0022 §2).
///
/// # Errors
///
/// [`VocabularyError::Malformed`] if `json` is not valid JSON or does not match the schema.
pub fn parse_submit_result(json: &str) -> Result<SubmitResult, VocabularyError> {
    serde_json::from_str(json).map_err(|error| VocabularyError::Malformed(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{Action, Field, Form, Panel, SelectOption, SubmitResult, Table, parse, parse_submit_result};

    fn sample_form() -> Form {
        Form {
            title: "Add note".to_owned(),
            fields: vec![
                Field::Text {
                    label: "Title".to_owned(),
                    name: "title".to_owned(),
                    placeholder: Some("Short summary".to_owned()),
                },
                Field::Textarea {
                    label: "Notes".to_owned(),
                    name: "notes".to_owned(),
                    placeholder: None,
                },
                Field::Number {
                    label: "Year".to_owned(),
                    name: "year".to_owned(),
                },
                Field::Date {
                    label: "When".to_owned(),
                    name: "when".to_owned(),
                },
                Field::Checkbox {
                    label: "Private".to_owned(),
                    name: "private".to_owned(),
                },
                Field::Select {
                    label: "Confidence".to_owned(),
                    name: "confidence".to_owned(),
                    options: vec![
                        SelectOption {
                            label: "Low".to_owned(),
                            value: "low".to_owned(),
                        },
                        SelectOption {
                            label: "High".to_owned(),
                            value: "high".to_owned(),
                        },
                    ],
                },
            ],
            actions: vec![
                Action {
                    id: "save".to_owned(),
                    label: "Save".to_owned(),
                },
                Action {
                    id: "preview".to_owned(),
                    label: "Preview".to_owned(),
                },
            ],
        }
    }

    #[test]
    fn form_actions_round_trip() {
        let panel = Panel::Form(sample_form());
        let json = serde_json::to_string(&panel).expect("serialize");
        let parsed = parse(&json).expect("parse");
        assert_eq!(parsed, panel);
    }

    #[test]
    fn panel_is_internally_tagged() {
        let form_json = serde_json::to_value(Panel::Form(sample_form())).expect("to value");
        assert_eq!(form_json["kind"], "form");
        assert_eq!(form_json["title"], "Add note");
        assert_eq!(form_json["fields"][0]["kind"], "text");
        assert_eq!(form_json["fields"][4]["kind"], "checkbox");
        assert_eq!(form_json["fields"][5]["kind"], "select");
        assert_eq!(form_json["actions"][0]["id"], "save");

        let table = Panel::Table(Table {
            title: "Preview".to_owned(),
            columns: vec!["Field".to_owned(), "Value".to_owned()],
            rows: vec![vec!["title".to_owned(), "Hi".to_owned()]],
        });
        let table_json = serde_json::to_value(&table).expect("to value");
        assert_eq!(table_json["kind"], "table");
    }

    #[test]
    fn textarea_and_date_parse() {
        let json = r#"{"kind":"form","title":"T","actions":[{"id":"a","label":"A"}],
            "fields":[{"kind":"textarea","label":"N","name":"n"},{"kind":"date","label":"D","name":"d"}]}"#;
        let Panel::Form(form) = parse(json).expect("parse") else {
            panic!("expected a form panel");
        };
        assert_eq!(
            form.fields[0],
            Field::Textarea {
                label: "N".to_owned(),
                name: "n".to_owned(),
                placeholder: None,
            }
        );
        assert_eq!(
            form.fields[1],
            Field::Date {
                label: "D".to_owned(),
                name: "d".to_owned(),
            }
        );
    }

    #[test]
    fn table_round_trips() {
        let panel = Panel::Table(Table {
            title: "preview-title".to_owned(),
            columns: vec!["col-field".to_owned(), "col-value".to_owned()],
            rows: vec![
                vec!["title".to_owned(), "Hello".to_owned()],
                vec!["year".to_owned(), "1900".to_owned()],
            ],
        });
        let json = serde_json::to_string(&panel).expect("serialize");
        assert_eq!(parse(&json).expect("parse"), panel);
    }

    #[test]
    fn submit_result_parses_success_with_panel() {
        let json = r#"{"kind":"success","message":"note-saved",
            "panel":{"kind":"table","title":"preview-title","columns":["col-field"],"rows":[["title"]]}}"#;
        let result = parse_submit_result(json).expect("parse");
        let SubmitResult::Success { message, panel } = result else {
            panic!("expected success");
        };
        assert_eq!(message.as_deref(), Some("note-saved"));
        assert!(matches!(panel, Some(Panel::Table(_))));
    }

    #[test]
    fn submit_result_parses_failure() {
        let result = parse_submit_result(r#"{"kind":"failure","message":"err-title-required"}"#).expect("parse");
        assert_eq!(
            result,
            SubmitResult::Failure {
                message: "err-title-required".to_owned(),
            }
        );
    }

    #[test]
    fn submit_result_success_defaults() {
        let result = parse_submit_result(r#"{"kind":"success"}"#).expect("parse");
        assert_eq!(
            result,
            SubmitResult::Success {
                message: None,
                panel: None,
            }
        );
    }

    #[test]
    fn text_placeholder_defaults_when_absent() {
        let json = r#"{"kind":"form","title":"T","actions":[{"id":"a","label":"A"}],
            "fields":[{"kind":"text","label":"L","name":"n"}]}"#;
        let Panel::Form(form) = parse(json).expect("parse") else {
            panic!("expected a form panel");
        };
        assert_eq!(
            form.fields[0],
            Field::Text {
                label: "L".to_owned(),
                name: "n".to_owned(),
                placeholder: None,
            }
        );
    }

    #[test]
    fn malformed_json_is_an_error() {
        assert!(parse("{ not json").is_err());
        assert!(parse(r#"{"title":"T"}"#).is_err());
        assert!(parse_submit_result(r#"{"kind":"bogus"}"#).is_err());
    }
}
