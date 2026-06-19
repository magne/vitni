//! Plugin-UI demo component (ADR 0012): returns a form description as a JSON string matching the
//! `genealogy-ui` vocabulary schema. The plugin emits the JSON directly rather than linking the
//! host's Rust types — the contract is the documented JSON shape (a non-Rust plugin would contribute
//! UI the same way). The host carries the payload opaquely; a framework renderer parses and renders.

wit_bindgen::generate!({
    world: "ui-panel",
    path: "../../crates/genealogy-plugin-host/wit",
});

use crate::genealogy::host_api::log;

/// The English form. Fields are internally tagged by `kind` (ADR 0012), matching
/// `genealogy_ui::vocabulary::Form`.
const FORM_EN: &str = r#"{
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

/// The Norwegian form — the plugin localizes its own labels for the locale the host passes.
const FORM_NO: &str = r#"{
  "title": "Legg til forskningsnotat",
  "submit": "Lagre notat",
  "fields": [
    { "kind": "text", "label": "Tittel", "name": "title", "placeholder": "Kort sammendrag" },
    { "kind": "text", "label": "Detalj", "name": "detail" },
    { "kind": "number", "label": "År", "name": "year" },
    { "kind": "checkbox", "label": "Privat", "name": "private" },
    {
      "kind": "select",
      "label": "Sikkerhet",
      "name": "confidence",
      "options": [
        { "label": "Lav", "value": "low" },
        { "label": "Normal", "value": "normal" },
        { "label": "Høy", "value": "high" }
      ]
    }
  ]
}"#;

struct UiPanelPlugin;

impl Guest for UiPanelPlugin {
    fn run_ui_panel(locale: String) -> Result<String, String> {
        log::log(log::Level::Info, &format!("emitting research-note form for {locale:?}"));
        // Match the language subtag: Norwegian (`no`/`nb`/`nn`, with or without a region) → Norwegian.
        let language = locale.split(['-', '_']).next().unwrap_or("");
        let form = match language {
            "no" | "nb" | "nn" => FORM_NO,
            _ => FORM_EN,
        };
        Ok(form.to_owned())
    }
}

export!(UiPanelPlugin);
