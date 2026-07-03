//! A reusable colour-picker dialog (ADR 0008 §5): a live hex preview plus a grid of preset swatches,
//! rendered as a centered overlay modal (its own `.overlay` + `.modal`, so it floats above the record
//! rather than in flow). No native `<input type=color>` (its chrome is unstyleable and inconsistent
//! across platforms); the picker is plain RSX so it matches the design system and stays
//! keyboard-accessible.
//!
//! Controlled: the call site owns the draft colour and passes it as `value`; the picker emits
//! `onselect` with the chosen hex (a preset click or a hex commit) and `oncancel` on dismiss. It
//! keeps a local buffer for the hex field so typing does not thrash the parent on every keystroke.

use dioxus::prelude::*;

use crate::components::{Button, ButtonVariant};
use crate::shell::focus_trap::keep_typing_local;

/// The preset palette offered in the swatch grid — a spread of hues (like the `WebAwesome` swatches)
/// plus neutrals, so the common tag colours are one click away while any colour stays reachable via
/// the hex field.
const PRESETS: [&str; 16] = [
    "#e5534b", "#e0884a", "#e0c84a", "#8a6d3b", "#2faa6a", "#4aa3e0", "#6cb6ff", "#b07cf0", "#d6409f", "#f78166",
    "#57ab5a", "#d6b32e", "#8a94a6", "#c2ccd6", "#e6edf3", "#1a2129",
];

/// A modal colour picker: preset swatches + a hex entry. Controlled by the call site.
#[component]
pub fn ColorPicker(
    /// Whether the dialog is shown.
    open: bool,
    /// The current colour (a CSS hex string), pre-selected / pre-filled.
    value: String,
    /// The already-localized dialog title.
    title: String,
    /// The already-localized "Presets" section label.
    presets_label: String,
    /// The already-localized hex-entry field label.
    hex_label: String,
    /// The already-localized confirm-button label.
    confirm_label: String,
    /// The already-localized cancel-button label.
    cancel_label: String,
    /// Fired with the chosen hex when the operator confirms (a preset click or the hex commit).
    onselect: EventHandler<String>,
    /// Fired when the operator dismisses the dialog without choosing.
    oncancel: EventHandler<()>,
) -> Element {
    let mut hex = use_signal(|| value.clone());
    if !open {
        return rsx! {};
    }
    rsx! {
        div { class: "overlay",
            div { class: "modal", role: "dialog", aria_modal: "true", aria_label: "{title}",
                div { class: "m-head", "{title}" }
                div { class: "m-body",
                    div { class: "stack",
                        // Live preview of the current selection.
                        div { class: "fact-row",
                            span {
                                class: "dot swatch-dot",
                                style: "width:40px;height:40px;border-radius:var(--r-md);background:{hex()};flex:none",
                            }
                            input {
                                class: "in",
                                r#type: "text",
                                id: "color-picker-hex",
                                name: "color-picker-hex",
                                aria_label: "{hex_label}",
                                style: "max-width:180px;font-family:var(--font-mono)",
                                value: "{hex()}",
                                oninput: move |event| hex.set(event.value()),
                                onkeydown: move |event| keep_typing_local(&event),
                            }
                        }
                        div { class: "field",
                            label { "{presets_label}" }
                            div { class: "color-grid", role: "listbox", aria_label: "{presets_label}",
                                for preset in PRESETS {
                                    {
                                        let preset = preset.to_owned();
                                        let chosen = preset.clone();
                                        let selected = hex().eq_ignore_ascii_case(&preset);
                                        rsx! {
                                            button {
                                                r#type: "button",
                                                class: if selected { "swatch-btn sel" } else { "swatch-btn" },
                                                role: "option",
                                                aria_selected: if selected { "true" } else { "false" },
                                                aria_label: "{preset}",
                                                title: "{preset}",
                                                onclick: move |_| hex.set(chosen.clone()),
                                                span {
                                                    class: "dot swatch-dot",
                                                    style: "width:32px;height:32px;border-radius:var(--r-md);background:{preset};flex:none",
                                                }
                                            }
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                div { class: "m-foot",
                    Button { label: cancel_label, variant: ButtonVariant::Ghost, onclick: move |_| oncancel.call(()) }
                    Button {
                        label: confirm_label,
                        variant: ButtonVariant::Primary,
                        onclick: move |_| onselect.call(hex()),
                    }
                }
            }
        }
    }
}
