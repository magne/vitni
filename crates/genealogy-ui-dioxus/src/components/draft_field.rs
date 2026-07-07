//! No-reflow record fields (`record-editing.html` §3/§4): one control that renders as a read box in
//! view mode and as an input (with per-field reset) in edit mode, so toggling a field between modes
//! moves no text. Generalized from the tag record editor's `.field-with-revert`.
//!
//! Controlled: each takes its current `value`, the committed `original`, and forwards edits/reset via
//! event handlers, so the call site's draft owns the state. A field whose `value` differs from its
//! `original` is *modified* — tinted and showing a reset control; a `locked` field renders a disabled
//! input rather than jumping to read text.

use dioxus::prelude::*;

use crate::components::IconButton;
use crate::shell::focus_trap::keep_typing_local;

/// A single-line text record field with per-field reset.
#[component]
pub fn DraftText(
    /// The field's already-localized label.
    label: String,
    /// The field's machine name / element id.
    name: String,
    /// Whether the record is in edit mode (inputs) or view mode (read box).
    editing: bool,
    /// The draft's current value.
    value: String,
    /// The committed value the field reverts to.
    original: String,
    /// The already-localized accessible name for the reset control.
    reset_label: String,
    /// An optional already-localized validation message (also drives `aria-invalid`).
    #[props(default)]
    error: Option<String>,
    /// An optional already-localized hint shown under the input in edit mode (e.g. "empty ⇒ generate").
    #[props(default)]
    hint: Option<String>,
    /// Whether the field is locked (§3): a disabled input in edit mode.
    #[props(default)]
    locked: bool,
    /// Whether to render the value in the monospace face (e.g. a human id or hex).
    #[props(default)]
    mono: bool,
    /// Fired on each input with the new value.
    oninput: EventHandler<String>,
    /// Fired when the reset control is pressed.
    onreset: EventHandler<()>,
) -> Element {
    let modified = value != original;
    let mono_style = if mono { "font-family:var(--font-mono)" } else { "" };
    if !editing {
        return rsx! {
            div { class: "field",
                label { r#for: "{name}", "{label}" }
                span { class: "val", style: "{mono_style}", "{value}" }
            }
        };
    }
    let mut input_class = String::from("in");
    if modified {
        input_class.push_str(" modified");
    }
    if error.is_some() {
        input_class.push_str(" invalid");
    }
    let aria_invalid = if error.is_some() { "true" } else { "false" };
    rsx! {
        div { class: "field",
            label { r#for: "{name}", "{label}" }
            div { class: "field-with-revert",
                input {
                    class: "{input_class}",
                    r#type: "text",
                    id: "{name}",
                    name: "{name}",
                    style: "{mono_style}",
                    value: "{value}",
                    disabled: locked,
                    aria_invalid,
                    oninput: move |event| oninput.call(event.value()),
                    onkeydown: move |event| keep_typing_local(&event),
                }
                if modified && !locked {
                    IconButton {
                        icon: "↺".to_owned(),
                        label: reset_label.clone(),
                        title: reset_label.clone(),
                        onclick: move |_| onreset.call(()),
                    }
                }
            }
            if let Some(message) = error {
                div { class: "field-error", "{message}" }
            }
            if let Some(hint) = hint {
                div { class: "field-hint", "{hint}" }
            }
        }
    }
}

/// A single-choice record field with per-field reset. View mode shows the selected option's label.
#[component]
pub fn DraftSelect(
    /// The field's already-localized label.
    label: String,
    /// The field's machine name / element id.
    name: String,
    /// Whether the record is in edit mode (a select) or view mode (read box).
    editing: bool,
    /// The currently-selected option value.
    value: String,
    /// The committed value the field reverts to.
    original: String,
    /// The already-localized accessible name for the reset control.
    reset_label: String,
    /// The selectable options, in display order.
    options: Vec<crate::components::SelectChoice>,
    /// Whether the field is locked (§3): a disabled select in edit mode.
    #[props(default)]
    locked: bool,
    /// Fired on change with the new value.
    onchange: EventHandler<String>,
    /// Fired when the reset control is pressed.
    onreset: EventHandler<()>,
) -> Element {
    let modified = value != original;
    let selected_label = options
        .iter()
        .find(|option| option.value == value)
        .map(|option| option.label.clone())
        .unwrap_or_default();
    if !editing {
        return rsx! {
            div { class: "field",
                label { r#for: "{name}", "{label}" }
                span { class: "val", "{selected_label}" }
            }
        };
    }
    let select_class = if modified { "in modified" } else { "in" };
    rsx! {
        div { class: "field",
            label { r#for: "{name}", "{label}" }
            div { class: "field-with-revert",
                select {
                    class: "{select_class}",
                    id: "{name}",
                    name: "{name}",
                    disabled: locked,
                    onchange: move |event| onchange.call(event.value()),
                    onkeydown: move |event| keep_typing_local(&event),
                    for option in options.iter() {
                        option { value: "{option.value}", selected: option.value == value, "{option.label}" }
                    }
                }
                if modified && !locked {
                    IconButton {
                        icon: "↺".to_owned(),
                        label: reset_label.clone(),
                        title: reset_label.clone(),
                        onclick: move |_| onreset.call(()),
                    }
                }
            }
        }
    }
}
