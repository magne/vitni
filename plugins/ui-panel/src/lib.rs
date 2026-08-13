//! Plugin-UI demo component (ADR 0012): returns a form description as a JSON string matching the
//! `vitni-ui` vocabulary schema. The labels are **Fluent message IDs**, not display text — the
//! frontend resolves them against this plugin's own catalogue (`i18n/<locale>/ui-panel.ftl`, ADR
//! 0003). The plugin emits the JSON directly rather than linking the host's Rust types, so a non-Rust
//! plugin contributes UI the same way. The host carries the payload opaquely.

wit_bindgen::generate!({
    world: "ui-panel",
    path: "../../crates/vitni-plugin-host/wit",
});

use crate::vitni::host_api::{commands, log};
use serde_json::Value;

/// The panel this plugin contributes (ADR 0022): a form of typed fields and two action buttons.
/// Every label is a message id resolved by the frontend; the ids are defined in this plugin's
/// `i18n/*/ui-panel.ftl` catalogues.
const PANEL_JSON: &str = r#"{
  "kind": "form",
  "title": "form-title",
  "fields": [
    { "kind": "text", "label": "f-title", "name": "title", "placeholder": "f-title-ph" },
    { "kind": "text", "label": "f-detail", "name": "detail" },
    { "kind": "textarea", "label": "f-notes", "name": "notes", "placeholder": "f-notes-ph" },
    { "kind": "number", "label": "f-year", "name": "year" },
    { "kind": "date", "label": "f-date", "name": "when" },
    { "kind": "checkbox", "label": "f-private", "name": "private" },
    {
      "kind": "select",
      "label": "f-confidence",
      "name": "confidence",
      "options": [
        { "label": "opt-low", "value": "low" },
        { "label": "opt-normal", "value": "normal" },
        { "label": "opt-high", "value": "high" }
      ]
    }
  ],
  "actions": [
    { "id": "save", "label": "act-save" },
    { "id": "preview", "label": "act-preview" }
  ]
}"#;

struct UiPanelPlugin;

impl Guest for UiPanelPlugin {
    fn run_ui_panel() -> Result<String, String> {
        log::log(log::Level::Info, "emitting research-note form (label ids)");
        Ok(PANEL_JSON.to_owned())
    }

    /// Handles an activated action (ADR 0022 §2). `values` is the form's field values keyed by field
    /// name. Returns a `submit-result` JSON string; a validation problem is a `failure`, an actual
    /// capability denial is a technical `err`.
    fn handle_action(action: String, values: String) -> Result<String, String> {
        let values: Value = serde_json::from_str(&values).map_err(|error| format!("invalid values payload: {error}"))?;
        match action.as_str() {
            "save" => save_note(&values),
            "preview" => Ok(preview(&values)),
            other => Err(format!("unknown action: {other}")),
        }
    }
}

/// Creates a research note from the submitted title, or reports a validation failure when it is
/// empty. A denied `commands` capability surfaces as a technical `err` (ADR 0022 §2, §3).
fn save_note(values: &Value) -> Result<String, String> {
    let title = values.get("title").and_then(Value::as_str).unwrap_or_default().trim();
    if title.is_empty() {
        return Ok(failure("err-title-required"));
    }
    log::log(log::Level::Info, "creating a note from the submitted form");
    match commands::create_note(title) {
        Ok(_human_id) => Ok(success("note-saved")),
        Err(error) => Err(format!("{error:?}")),
    }
}

/// Echoes the submitted values as a read-only table panel (ADR 0022 §1): one row per field, cells
/// are literal data (never resolved), the title and column headers are message ids.
fn preview(values: &Value) -> String {
    let mut rows: Vec<Value> = Vec::new();
    if let Some(object) = values.as_object() {
        for (name, value) in object {
            rows.push(serde_json::json!([name, cell_text(value)]));
        }
    }
    serde_json::json!({
        "kind": "success",
        "panel": {
            "kind": "table",
            "title": "preview-title",
            "columns": ["col-field", "col-value"],
            "rows": rows,
        }
    })
    .to_string()
}

/// Renders one submitted value as a display string for the preview table.
fn cell_text(value: &Value) -> String {
    match value {
        Value::String(text) => text.clone(),
        Value::Bool(flag) => flag.to_string(),
        Value::Number(number) => number.to_string(),
        Value::Null => String::new(),
        Value::Array(_) | Value::Object(_) => value.to_string(),
    }
}

/// Builds a `{"kind":"success","message":<id>}` submit-result.
fn success(message_id: &str) -> String {
    serde_json::json!({ "kind": "success", "message": message_id }).to_string()
}

/// Builds a `{"kind":"failure","message":<id>}` submit-result (validation feedback, ADR 0022 §2).
fn failure(message_id: &str) -> String {
    serde_json::json!({ "kind": "failure", "message": message_id }).to_string()
}

export!(UiPanelPlugin);
