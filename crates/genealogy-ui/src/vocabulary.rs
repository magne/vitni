//! The plugin-UI vocabulary (ADR 0012): a small, serializable form description a plugin emits and a
//! framework renderer interprets.
//!
//! A plugin returns the form as a JSON document (it is a sandboxed component that cannot link these
//! types — the contract is the documented JSON shape, not this crate). [`parse`] validates that JSON
//! into a [`Form`]. The encoding follows the project's **internally-tagged** convention (a `kind`
//! discriminator per field), matching the event encoding (ADR 0004 §4), and is **additive**: new
//! field kinds are appended without breaking older renderers or plugins.

use serde::{Deserialize, Serialize};

/// A single-screen form: a title, an ordered list of typed fields, and a submit-button label.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Form {
    /// The form's heading.
    pub title: String,
    /// The fields, rendered top to bottom in order.
    pub fields: Vec<Field>,
    /// The submit-button label.
    pub submit: String,
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
    /// A numeric input.
    Number {
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

/// A failure to parse a plugin's form JSON against the vocabulary schema (ADR 0012).
#[derive(Debug, thiserror::Error)]
pub enum VocabularyError {
    /// The JSON did not match the [`Form`] schema.
    #[error("malformed plugin form: {0}")]
    Malformed(String),
}

/// Parses a plugin-supplied JSON document into a [`Form`].
///
/// # Errors
///
/// [`VocabularyError::Malformed`] if `json` is not valid JSON or does not match the schema.
pub fn parse(json: &str) -> Result<Form, VocabularyError> {
    serde_json::from_str(json).map_err(|error| VocabularyError::Malformed(error.to_string()))
}

#[cfg(test)]
mod tests {
    use super::{Field, Form, SelectOption, parse};

    fn sample() -> Form {
        Form {
            title: "Add note".to_owned(),
            fields: vec![
                Field::Text {
                    label: "Title".to_owned(),
                    name: "title".to_owned(),
                    placeholder: Some("Short summary".to_owned()),
                },
                Field::Number {
                    label: "Year".to_owned(),
                    name: "year".to_owned(),
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
            submit: "Save".to_owned(),
        }
    }

    #[test]
    fn round_trips_through_json() {
        let form = sample();
        let json = serde_json::to_string(&form).expect("serialize");
        let parsed = parse(&json).expect("parse");
        assert_eq!(parsed, form);
    }

    #[test]
    fn fields_are_internally_tagged() {
        let json = serde_json::to_value(sample()).expect("to value");
        assert_eq!(json["fields"][0]["kind"], "text");
        assert_eq!(json["fields"][2]["kind"], "checkbox");
        assert_eq!(json["fields"][3]["kind"], "select");
    }

    #[test]
    fn text_placeholder_defaults_when_absent() {
        let json = r#"{"title":"T","submit":"S","fields":[{"kind":"text","label":"L","name":"n"}]}"#;
        let form = parse(json).expect("parse");
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
    }
}
