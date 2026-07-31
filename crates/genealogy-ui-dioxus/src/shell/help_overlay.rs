//! The keyboard-shortcuts help sheet (`?`).
//!
//! Renders the *resolved* shortcut map (`genealogy_ui::resolved_shortcuts`, ADR 0030 §1 — so a
//! rebound Global chord shows up here for free) as a three-column grid (Global / Go to / Within
//! screen). Each row pairs a localized description with its chord drawn as `kbd` glyphs (decorative —
//! the description carries the meaning). Closes on `Esc` or a click outside; focus rests on the close
//! control.

use dioxus::prelude::*;
use genealogy_ui::{Chord, Key as ChordKey, Modifier, Shortcut, ShortcutGroup, navigation_shortcuts};

use crate::shell::focus_trap::{DialogFocus, dismiss_on_escape, trap_tab};
use crate::shell::nav_state::{NavState, Overlay};
use crate::shell::{ChromeCtx, resolved_shortcuts_from_context};

/// The help overlay, rendered only while [`Overlay::Help`] is open.
#[component]
pub fn HelpOverlay() -> Element {
    let mut nav = use_context::<NavState>();
    let chrome = use_context::<ChromeCtx>();
    if *nav.overlay.read() != Overlay::Help {
        return rsx! {};
    }
    let resolved = resolved_shortcuts_from_context();
    rsx! {
        div {
            class: "overlay",
            onclick: move |_| nav.close_overlay(),
            onkeydown: move |event: KeyboardEvent| dismiss_on_escape(&event, || nav.dismiss_topmost()),
            div {
                class: "help-sheet",
                role: "dialog",
                aria_modal: "true",
                aria_label: "{chrome.0.help_title()}",
                tabindex: "-1",
                "data-focus-trap": "true",
                onclick: move |event| event.stop_propagation(),
                onkeydown: move |event| trap_tab(&event),
                // `autofocus` on the close button below does not take in the live webview, which left
                // focus on `body` — outside this subtree, so `Esc` reached no handler and the sheet
                // could not be closed from the keyboard at all. `DialogFocus` moves focus in (and
                // restores it on close) the same way the `Modal` layer does.
                DialogFocus {}
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
                    HelpColumn { group: ShortcutGroup::Global, resolved: resolved.clone() }
                    HelpColumn { group: ShortcutGroup::Navigation, resolved: resolved.clone() }
                    HelpColumn { group: ShortcutGroup::WithinScreen, resolved }
                }
            }
        }
    }
}

/// One column of the help grid: a heading and its shortcut rows. `resolved` is the live resolved map
/// (ignored by the `Navigation` group, which is not rebindable and reads
/// [`navigation_shortcuts`] instead).
#[component]
fn HelpColumn(group: ShortcutGroup, resolved: Vec<Shortcut>) -> Element {
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
                        for entry in resolved.into_iter().filter(|entry| entry.group == group) {
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

/// The concatenated display string for a chord (e.g. `⌘K`), for inline hint text such as the command
/// palette's "`⌘K` from anywhere" — contrast [`render_chord`], which draws it as separate `kbd` cells
/// for the grid.
#[must_use]
pub(crate) fn chord_display(chord: Chord) -> String {
    let mut text = modifier_glyph(chord.modifier).unwrap_or_default();
    text.push_str(&key_glyph(chord.key));
    text
}

/// Draws a chord as `kbd` glyphs (e.g. `⌘ K`, `⌘⇧ Z`, `↑`).
fn render_chord(chord: Chord) -> Element {
    let modifier = modifier_glyph(chord.modifier);
    let key = key_glyph(chord.key);
    rsx! {
        if let Some(modifier) = modifier {
            kbd { "{modifier}" }
        }
        kbd { "{key}" }
    }
}

/// The modifier glyph for a chord (e.g. `⌘`, `⌘⇧`, `⌘⌥`), or `None` when no modifier is held. `Alt`
/// composes independently of the primary modifier and Shift (ADR 0030 §5).
fn modifier_glyph(modifier: Modifier) -> Option<String> {
    if !modifier.command && !modifier.shift && !modifier.alt {
        return None;
    }
    let mut glyph = String::new();
    if modifier.command {
        glyph.push_str(primary_glyph());
    }
    if modifier.shift {
        glyph.push('⇧');
    }
    if modifier.alt {
        glyph.push_str(if cfg!(target_os = "macos") { "⌥" } else { "Alt+" });
    }
    Some(glyph)
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
