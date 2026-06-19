//! Plugin-UI demo component (ADR 0012): returns a form description as a JSON string matching the
//! `genealogy-ui` vocabulary schema. The plugin emits the JSON directly rather than linking the
//! host's Rust types — the contract is the documented JSON shape (a non-Rust plugin would contribute
//! UI the same way). The host carries the payload opaquely; a framework renderer parses and renders.

wit_bindgen::generate!({
    world: "ui-panel",
    path: "../../crates/genealogy-plugin-host/wit",
});

use crate::genealogy::host_api::log;

/// The form this plugin contributes. Fields are internally tagged by `kind` (ADR 0012), matching
/// `genealogy_ui::vocabulary::Form`.
const FORM_JSON: &str = r#"{
  "title": "Add research note",
  "submit": "Save note",
  "fields": [
    { "kind": "text", "label": "Title", "name": "title", "placeholder": "Short summary" },
    { "kind": "text", "label": "Detail", "name": "detail" },
    { "kind": "number", "label": "Year", "name": "year" },
    { "kind": "checkbox", "label": "Private", "name": "private" },
    {
      "kind": "select",
      "label": "Confidence",
      "name": "confidence",
      "options": [
        { "label": "Low", "value": "low" },
        { "label": "Normal", "value": "normal" },
        { "label": "High", "value": "high" }
      ]
    }
  ]
}"#;

struct UiPanelPlugin;

impl Guest for UiPanelPlugin {
    fn run_ui_panel() -> Result<String, String> {
        log::log(log::Level::Info, "emitting research-note form");
        Ok(FORM_JSON.to_owned())
    }
}

export!(UiPanelPlugin);
