//! The `<input>`/`<textarea>` behavior core (ADR 0008 §5): with
//! [`select_input`](crate::components::select_input), one of the two files allowed to render a raw
//! form element. Every text control in the app composes [`TextInput`] so the global-shortcut guard
//! [`keep_typing_local`] is wired exactly once (fixing "global keys fire inside text controls"); the
//! `input-guard` xtask lint forbids raw form elements anywhere else.
//!
//! **Controlled, and not optionally so: the call site owns the value and forwards edits through
//! `oninput`.** Omitting `value` does not merely leave the field uncontrolled — it makes every
//! re-render *erase what the operator has typed*. `value` is a **volatile** attribute in
//! `dioxus-html`, so `dioxus-core` re-writes it to the DOM on every diff whether it changed or not,
//! and a `None` value is written as a *removal* whose interpreter shim runs `node.value = ""`. A field
//! whose parent re-renders while it has focus therefore loses its text, silently: the signal still
//! holds the old string, so the next keystroke's `event.value()` is that one character alone and
//! overwrites it. That is how the provenance reason field committed only the last character typed —
//! the whole-record save path's one operator-supplied "why".
//!
//! Search widgets that must handle `Arrow`/`Enter`/`Escape` first pass an `onkeydown_extra` handler —
//! those are non-character keys, so it composes with the guard (which only swallows unmodified typing).
//! Presentational attributes (`role`, `aria-*`, `style`, `id`, `autofocus`, `inputmode`, …) flow
//! through the `extends = GlobalAttributes` spread; the few input/textarea-specific attributes the
//! call sites need (`name`, `placeholder`, `min`, `max`, `rows`) are typed props.

use dioxus::prelude::*;

use crate::shell::focus_trap::keep_typing_local;

/// The native `type` of a [`TextInput`] rendered as an `<input>` (ignored when `multiline`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum TextInputKind {
    /// A single-line text field (`type="text"`).
    #[default]
    Text,
    /// A numeric field (`type="number"`).
    Number,
    /// A date field (`type="date"`, ISO-8601 wire value).
    Date,
    /// A slider field (`type="range"`; the Geography tool's time slider).
    Range,
}

impl TextInputKind {
    /// The HTML `type` attribute value.
    const fn as_str(self) -> &'static str {
        match self {
            TextInputKind::Text => "text",
            TextInputKind::Number => "number",
            TextInputKind::Date => "date",
            TextInputKind::Range => "range",
        }
    }
}

/// The behavior core for every text/number/date field and textarea. Wires the typing guard once.
#[component]
pub fn TextInput(
    /// The controlled value. Accepts a `String` or an `Option<String>`; `None` omits the attribute,
    /// which **blanks the live field on every re-render** — see the module header before doing it.
    #[props(default, into)]
    value: Option<String>,
    /// Fired on each input event (omit for a display-only field).
    #[props(default)]
    oninput: Option<EventHandler<FormEvent>>,
    /// The native input `type` (ignored when `multiline`).
    #[props(default)]
    kind: TextInputKind,
    /// Render a `<textarea>` rather than an `<input>`.
    #[props(default)]
    multiline: bool,
    /// Whether the control is disabled.
    #[props(default)]
    disabled: bool,
    /// Whether the value is invalid: sets `aria-invalid` and appends `" invalid"` to the class.
    #[props(default)]
    invalid: bool,
    /// The base CSS class (defaults to `"in"`).
    #[props(default, into)]
    class: Option<String>,
    /// The `name` attribute (and, for label association, the element id when set).
    #[props(default, into)]
    name: Option<String>,
    /// The `placeholder` attribute.
    #[props(default, into)]
    placeholder: Option<String>,
    /// The `min` attribute (numeric fields).
    #[props(default, into)]
    min: Option<String>,
    /// The `max` attribute (numeric fields).
    #[props(default, into)]
    max: Option<String>,
    /// The `rows` attribute (textarea).
    #[props(default, into)]
    rows: Option<String>,
    /// A handler run *before* the typing guard, for search widgets that consume `Arrow`/`Enter`/`Esc`.
    #[props(default)]
    onkeydown_extra: Option<EventHandler<KeyboardEvent>>,
    /// Fired on blur.
    #[props(default)]
    onblur: Option<EventHandler<FocusEvent>>,
    /// Fired on focus.
    #[props(default)]
    onfocus: Option<EventHandler<FocusEvent>>,
    /// Fired on `focusin`.
    #[props(default)]
    onfocusin: Option<EventHandler<FocusEvent>>,
    /// Fired on `focusout`.
    #[props(default)]
    onfocusout: Option<EventHandler<FocusEvent>>,
    /// Presentational attributes (`role`, `aria-*`, `style`, `id`, `autofocus`, `inputmode`, …).
    #[props(extends = GlobalAttributes)]
    attributes: Vec<Attribute>,
) -> Element {
    let mut class = class.unwrap_or_else(|| "in".to_owned());
    if invalid {
        class.push_str(" invalid");
    }
    let aria_invalid = if invalid { "true" } else { "false" };
    let on_key = move |event: KeyboardEvent| {
        if let Some(extra) = &onkeydown_extra {
            extra.call(event.clone());
        }
        keep_typing_local(&event);
    };
    let on_input = move |event: FormEvent| {
        if let Some(oninput) = &oninput {
            oninput.call(event);
        }
    };
    let on_blur = move |event: FocusEvent| {
        if let Some(onblur) = &onblur {
            onblur.call(event);
        }
    };
    let on_focus = move |event: FocusEvent| {
        if let Some(onfocus) = &onfocus {
            onfocus.call(event);
        }
    };
    let on_focus_in = move |event: FocusEvent| {
        if let Some(onfocusin) = &onfocusin {
            onfocusin.call(event);
        }
    };
    let on_focus_out = move |event: FocusEvent| {
        if let Some(onfocusout) = &onfocusout {
            onfocusout.call(event);
        }
    };
    if multiline {
        return rsx! {
            textarea {
                class: "{class}",
                name,
                rows,
                disabled,
                aria_invalid,
                value,
                oninput: on_input,
                onkeydown: on_key,
                onblur: on_blur,
                onfocus: on_focus,
                onfocusin: on_focus_in,
                onfocusout: on_focus_out,
                ..attributes,
            }
        };
    }
    rsx! {
        input {
            class: "{class}",
            r#type: kind.as_str(),
            name,
            placeholder,
            min,
            max,
            disabled,
            aria_invalid,
            value,
            oninput: on_input,
            onkeydown: on_key,
            onblur: on_blur,
            onfocus: on_focus,
            onfocusin: on_focus_in,
            onfocusout: on_focus_out,
            ..attributes,
        }
    }
}
