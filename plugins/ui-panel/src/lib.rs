//! Plugin-UI demo component (ADR 0012): returns a form description as a JSON string matching the
//! `genealogy-ui` vocabulary schema. The labels are **Fluent message IDs**, not display text — the
//! frontend resolves them against this plugin's own catalogue (`i18n/<locale>/ui-panel.ftl`, ADR
//! 0003). The plugin emits the JSON directly rather than linking the host's Rust types, so a non-Rust
//! plugin contributes UI the same way. The host carries the payload opaquely.

wit_bindgen::generate!({
    world: "ui-panel",
    path: "../../crates/genealogy-plugin-host/wit",
});

use crate::genealogy::host_api::log;

/// The form this plugin contributes. Every label is a message id resolved by the frontend; the ids
/// are defined in this plugin's `i18n/*/ui-panel.ftl` catalogues.
const FORM_JSON: &str = r#"{
  "title": "form-title",
  "submit": "form-submit",
  "fields": [
    { "kind": "text", "label": "f-title", "name": "title", "placeholder": "f-title-ph" },
    { "kind": "text", "label": "f-detail", "name": "detail" },
    { "kind": "number", "label": "f-year", "name": "year" },
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
  ]
}"#;

struct UiPanelPlugin;

impl Guest for UiPanelPlugin {
    fn run_ui_panel() -> Result<String, String> {
        log::log(log::Level::Info, "emitting research-note form (label ids)");
        Ok(FORM_JSON.to_owned())
    }
}

export!(UiPanelPlugin);
