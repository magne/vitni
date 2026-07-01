//! Focus containment for the shell overlays.
//!
//! PR2's overlays (the command palette and the help sheet) each have a single primary focusable
//! control, so focus enters declaratively via `autofocus` on that control and is contained by
//! [`trap_tab`]: while the overlay is open, `Tab`/`Shift+Tab` are swallowed so focus cannot escape
//! to the inert page behind. `Esc` is not handled here — it bubbles to the shell's central keyboard
//! dispatcher, which closes the overlay.
//!
//! A general multi-element focus trap (cycling across several controls, restore-on-close for the
//! side panel / modal) lands with the side-panel editing in the Person slice (PR4), where it is
//! exercised by a real screen.

use dioxus::prelude::*;

/// Swallows `Tab`/`Shift+Tab` so focus stays within a single-focusable overlay.
///
/// Attach to the overlay's dialog root `onkeydown`. Other keys (including `Esc`) are left to bubble
/// to the shell dispatcher.
pub fn trap_tab(event: &KeyboardEvent) {
    if event.key() == Key::Tab {
        event.prevent_default();
    }
}

/// `true` when an unmodified character key should stay in the focused text input (so `g`/`?`/…
/// do not reach the shell's global shortcut dispatcher). `Esc`, `Tab`, and modifier chords
/// (`⌘K`, …) return `false` so they still bubble.
fn is_local_typing(key: &Key, modifiers: Modifiers) -> bool {
    if let Key::Character(_) = key {
        !(modifiers.meta() || modifiers.ctrl())
    } else {
        false
    }
}

/// Keeps unmodified character typing inside a text input (so `g`/`?` do not trigger shortcuts),
/// while letting `Esc`, `Tab`, and modifier chords (`⌘K`, …) bubble to the shell dispatcher.
pub fn keep_typing_local(event: &KeyboardEvent) {
    if is_local_typing(&event.key(), event.modifiers()) {
        event.stop_propagation();
    }
}

#[cfg(test)]
mod tests {
    use super::is_local_typing;
    use dioxus::prelude::{Key, Modifiers};

    #[test]
    fn unmodified_characters_stay_local() {
        for character in ["t", "g", "?", " "] {
            let key = Key::Character(character.to_string());
            assert!(is_local_typing(&key, Modifiers::empty()));
        }
    }

    #[test]
    fn control_or_meta_chords_bubble() {
        let key = Key::Character("k".to_string());
        assert!(!is_local_typing(&key, Modifiers::CONTROL));
        assert!(!is_local_typing(&key, Modifiers::META));
    }

    #[test]
    fn non_character_keys_bubble() {
        for key in [Key::Escape, Key::Tab, Key::ArrowDown] {
            assert!(!is_local_typing(&key, Modifiers::empty()));
        }
    }
}
