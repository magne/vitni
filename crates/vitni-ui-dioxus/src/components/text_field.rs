//! Composed labelled text field built on the [`TextInput`] core: a `<label>` associated to the
//! control, the guarded input, an optional per-field revert control (shown when `modified`), an
//! optional validation error and hint, and an adornment `children` slot for trailing controls (e.g.
//! the number stepper's ▲/▼). The bespoke record-ish fields (tag name/priority, DNA shared-cM) share
//! this so their `.field` + `.field-with-revert` / `.number-stepper` markup lives in one place.
//!
//! Two shapes, chosen by `label_width`: unset (the default) is the stacked `div.field` with the label
//! above the control, which is what the settings forms draw (`preferences.html:181-193`); set is the
//! one-line [`FactRow`] every *record* page draws (`record-editing.html:47-99`, `tag.html:133`). The
//! knob is opt-in rather than defaulted because this field serves both, and a record page's label
//! column width is its own (see [`FactRow`]).

use dioxus::prelude::*;

use crate::components::text_input::{TextInput, TextInputKind};
use crate::components::{FactRow, IconButton};

/// A label associated to a form control by id.
#[component]
pub fn InputLabel(
    /// The already-localized label text.
    label: String,
    /// The associated control's id.
    name: String,
) -> Element {
    rsx! {
        label { r#for: "{name}", "{label}" }
    }
}

/// The validation error (`.field-error`) or hint (`.field-hint`) shown under a field.
#[component]
pub fn FieldMessage(
    /// An optional already-localized error message.
    #[props(default)]
    error: Option<String>,
    /// An optional already-localized hint.
    #[props(default)]
    hint: Option<String>,
) -> Element {
    rsx! {
        if let Some(message) = error {
            div { class: "field-error", "{message}" }
        }
        if let Some(hint) = hint {
            div { class: "field-hint", "{hint}" }
        }
    }
}

/// A labelled text field over the [`TextInput`] core (see module docs).
#[component]
pub fn TextField(
    /// The field's already-localized label.
    label: String,
    /// The control's machine name / element id (label association).
    name: String,
    /// The label column's width in pixels, which also picks the shape: `Some` renders the one-line
    /// [`FactRow`] a record page draws, `None` the stacked `div.field` a settings form draws.
    #[props(default)]
    label_width: Option<u32>,
    /// The controlled value.
    value: String,
    /// Fired on each input event, with the form event.
    oninput: EventHandler<FormEvent>,
    /// The native input `type`.
    #[props(default)]
    kind: TextInputKind,
    /// Whether to render the value in the monospace face.
    #[props(default)]
    mono: bool,
    /// Whether the value is invalid (drives `aria-invalid` and the error border).
    #[props(default)]
    invalid: bool,
    /// An optional already-localized validation message shown under the field.
    #[props(default)]
    error: Option<String>,
    /// An optional already-localized hint shown under the field.
    #[props(default)]
    hint: Option<String>,
    /// Whether the control is disabled.
    #[props(default)]
    disabled: bool,
    /// Whether the control autofocuses on mount.
    #[props(default)]
    autofocus: bool,
    /// The `inputmode` attribute (e.g. `"numeric"`, `"decimal"`).
    #[props(default, into)]
    inputmode: Option<String>,
    /// The container class wrapping the input + revert + adornment (defaults to
    /// `"field-with-revert"`; e.g. `"number-stepper"`).
    #[props(default, into)]
    container_class: Option<String>,
    /// The input's base CSS class (defaults to `"in"`; e.g. `"stepper-value"`).
    #[props(default, into)]
    input_class: Option<String>,
    /// Whether the field differs from its committed value (gates the revert control).
    #[props(default)]
    modified: bool,
    /// The already-localized accessible name for the revert control.
    #[props(default, into)]
    reset_label: Option<String>,
    /// Fired when the revert control is pressed (omit to hide it).
    #[props(default)]
    onreset: Option<EventHandler<()>>,
    /// Fired on blur.
    #[props(default)]
    onblur: Option<EventHandler<FocusEvent>>,
    /// Trailing adornment controls rendered inside the container after the input/revert.
    #[props(default)]
    children: Element,
) -> Element {
    let style = if mono {
        Some("font-family:var(--font-mono)")
    } else {
        None
    };
    let show_revert = modified && onreset.is_some() && reset_label.is_some();
    let revert = rsx! {
        if show_revert {
            IconButton {
                icon: "↺".to_owned(),
                label: reset_label.clone().unwrap_or_default(),
                title: reset_label.clone().unwrap_or_default(),
                onclick: move |_| {
                    if let Some(onreset) = &onreset {
                        onreset.call(());
                    }
                },
            }
        }
    };
    let field = rsx! {
        TextInput {
            id: "{name}",
            name: name.clone(),
            value,
            kind,
            invalid,
            disabled,
            autofocus,
            inputmode,
            class: input_class.unwrap_or_else(|| "in".to_owned()),
            style,
            oninput: move |event| oninput.call(event),
            onblur: move |event| {
                if let Some(onblur) = &onblur {
                    onblur.call(event);
                }
            },
        }
    };
    let mut container = container_class.unwrap_or_else(|| "field-with-revert".to_owned());
    if invalid {
        container.push_str(" invalid");
    }
    let control = rsx! {
        div { class: "{container}",
            {field}
            {revert}
            {children}
        }
        FieldMessage { error, hint }
    };
    let Some(label_width) = label_width else {
        return rsx! {
            div { class: "field",
                InputLabel { label, name: name.clone() }
                {control}
            }
        };
    };
    rsx! {
        FactRow { label, label_width, name: name.clone(),
            div { class: "grow", {control} }
        }
    }
}
