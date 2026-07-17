//! The `<select>` behavior core (ADR 0008 §5): with [`text_input`](crate::components::text_input),
//! one of the two files allowed to render a raw form element. Every dropdown in the app composes
//! [`SelectInput`] so the global-shortcut typing guard [`keep_typing_local`] is wired exactly once
//! (a keystroke that opens the native list must not also reach the shell dispatcher); the
//! `input-guard` xtask lint forbids raw `<select>` anywhere else.
//!
//! Controlled: the call site owns the selected value and forwards changes through `onchange`.
//! Presentational attributes (`aria-*`, `style`, `id`, …) flow through the `extends = GlobalAttributes`
//! spread; `name` is a typed prop (it is form-specific, not a global attribute).

use dioxus::prelude::*;

use crate::shell::focus_trap::keep_typing_local;

/// One option in a [`SelectInput`] / [`Select`](crate::components::Select).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SelectChoice {
    /// The value submitted when this option is chosen.
    pub value: String,
    /// The visible, already-localized label.
    pub label: String,
}

/// The behavior core for every `<select>`. Wires the typing guard once.
#[component]
pub fn SelectInput(
    /// The selectable options, in display order.
    options: Vec<SelectChoice>,
    /// The currently selected option value (an option is selected when its `value` matches).
    #[props(default)]
    selected: String,
    /// Fired on change (omit for a display-only select).
    #[props(default)]
    onchange: Option<EventHandler<FormEvent>>,
    /// Whether the control is disabled.
    #[props(default)]
    disabled: bool,
    /// The base CSS class (defaults to `"in"`).
    #[props(default, into)]
    class: Option<String>,
    /// The `name` attribute (and, for label association, the element id when set).
    #[props(default, into)]
    name: Option<String>,
    /// Presentational attributes (`aria-*`, `style`, `id`, …).
    #[props(extends = GlobalAttributes)]
    attributes: Vec<Attribute>,
) -> Element {
    let class = class.unwrap_or_else(|| "in".to_owned());
    let on_change = move |event: FormEvent| {
        if let Some(onchange) = &onchange {
            onchange.call(event);
        }
    };
    rsx! {
        select {
            class: "{class}",
            name,
            disabled,
            onchange: on_change,
            onkeydown: move |event| keep_typing_local(&event),
            ..attributes,
            for option in options.iter() {
                option {
                    value: "{option.value}",
                    selected: option.value == selected,
                    "{option.label}"
                }
            }
        }
    }
}
