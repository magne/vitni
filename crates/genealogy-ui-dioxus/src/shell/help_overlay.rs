//! The keyboard-shortcuts help sheet (`?`).
//!
//! Renders the framework-neutral shortcut map (`genealogy_ui::shortcuts`) as a three-column grid
//! (Global / Go to / Within screen). Each row pairs a localized description with its chord drawn as
//! `kbd` glyphs (decorative — the description carries the meaning). Closes on `Esc` or a click
//! outside; focus rests on the close control.

use dioxus::prelude::*;
use genealogy_ui::{Chord, Key as ChordKey, Modifier, ShortcutGroup, navigation_shortcuts, shortcuts};

use crate::shell::ChromeCtx;
use crate::shell::focus_trap::trap_tab;
use crate::shell::nav_state::{NavState, Overlay};

/// The help overlay, rendered only while [`Overlay::Help`] is open.
#[component]
pub fn HelpOverlay() -> Element {
    let mut nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
    if *nav.overlay.read() != Overlay::Help {
        return rsx! {};
    }
    rsx! {
        div { class: "overlay", onclick: move |_| nav.close_overlay(),
            div {
                class: "help-sheet",
                role: "dialog",
                aria_modal: "true",
                aria_label: "{chrome.0.help_title()}",
                onclick: move |event| event.stop_propagation(),
                onkeydown: move |event| trap_tab(&event),
                div { class: "h-head",
                    h3 { "{chrome.0.help_title()}" }
                    span { class: "spacer" }
                    button {
                        class: "icon-btn",
                        autofocus: true,
                        aria_label: "{chrome.0.close()}",
                        onclick: move |_| nav.close_overlay(),
                        "✕"
                    }
                }
                div { class: "h-body",
                    HelpColumn { group: ShortcutGroup::Global }
                    HelpColumn { group: ShortcutGroup::Navigation }
                    HelpColumn { group: ShortcutGroup::WithinScreen }
                }
            }
        }
    }
}

/// One column of the help grid: a heading and its shortcut rows.
#[component]
fn HelpColumn(group: ShortcutGroup) -> Element {
    let chrome = use_context::<ChromeCtx>();
    rsx! {
        div { class: "help-col",
            h4 { "{chrome.0.help_column(group)}" }
            div { class: "shortcut-list",
                match group {
                    ShortcutGroup::Navigation => rsx! {
                        for row in navigation_shortcuts() {
                            div { class: "shortcut-row",
                                span { "{chrome.0.shortcut_label(row.label_id)}" }
                                span { class: "keys", aria_hidden: "true",
                                    kbd { "g" }
                                    kbd { "{row.key}" }
                                }
                            }
                        }
                    },
                    ShortcutGroup::Global | ShortcutGroup::WithinScreen => rsx! {
                        for entry in shortcuts().into_iter().filter(|entry| entry.group == group) {
                            div { class: "shortcut-row",
                                span { "{chrome.0.shortcut_label(entry.label_id)}" }
                                span { class: "keys", aria_hidden: "true", {render_chord(entry.chord)} }
                            }
                        }
                    },
                }
            }
        }
    }
}

/// Draws a chord as `kbd` glyphs (e.g. `⌘ K`, `⌘⇧ Z`, `↑`).
fn render_chord(chord: Chord) -> Element {
    let modifier = match chord.modifier {
        Modifier::None => None,
        Modifier::Command => Some(primary_glyph().to_owned()),
        Modifier::CommandShift => Some(format!("{}⇧", primary_glyph())),
    };
    let key = key_glyph(chord.key);
    rsx! {
        if let Some(modifier) = modifier {
            kbd { "{modifier}" }
        }
        kbd { "{key}" }
    }
}

/// The primary modifier glyph: `⌘` on macOS, `Ctrl` elsewhere.
fn primary_glyph() -> &'static str {
    if cfg!(target_os = "macos") { "⌘" } else { "Ctrl" }
}

/// The display glyph for a chord key.
fn key_glyph(key: ChordKey) -> String {
    match key {
        ChordKey::Char(character) => character.to_uppercase().to_string(),
        ChordKey::Digit(digit) => digit.to_string(),
        ChordKey::DigitRange => "1…9".to_owned(),
        ChordKey::Question => "?".to_owned(),
        ChordKey::Escape => "Esc".to_owned(),
        ChordKey::Enter => "↵".to_owned(),
        ChordKey::ArrowUp => "↑".to_owned(),
        ChordKey::ArrowDown => "↓".to_owned(),
        ChordKey::ArrowLeft => "←".to_owned(),
        ChordKey::ArrowRight => "→".to_owned(),
        ChordKey::Home => "Home".to_owned(),
        ChordKey::End => "End".to_owned(),
        ChordKey::BracketLeft => "[".to_owned(),
        ChordKey::BracketRight => "]".to_owned(),
    }
}
