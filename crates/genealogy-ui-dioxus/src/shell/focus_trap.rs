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

/// Keeps unmodified character typing inside a text input (so `g`/`?` do not trigger shortcuts),
/// while letting `Esc`, `Tab`, and modifier chords (`⌘K`, …) bubble to the shell dispatcher.
pub fn keep_typing_local(event: &KeyboardEvent) {
    if let Key::Character(_) = event.key() {
        let modifiers = event.modifiers();
        if !(modifiers.meta() || modifiers.ctrl()) {
            event.stop_propagation();
        }
    }
}
