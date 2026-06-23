//! The central keyboard dispatcher.
//!
//! One `onkeydown` on the shell root interprets the framework-neutral shortcut map
//! (`genealogy_ui::shortcuts`) into shell actions: open the palette, toggle help, navigate via the
//! `g`-prefix, switch record tabs, and close overlays. The primary modifier is `⌘` on macOS and
//! `Ctrl` elsewhere. Within-screen keys (↑/↓, Enter, `[`/`]`, …) are *not* handled here — the
//! focused list/tab widget owns them.

use std::time::{Duration, Instant};

use dioxus::prelude::*;
use genealogy_ui::{Category, Destination};

use crate::shell::nav_state::{NavState, Overlay};

/// How long after pressing `g` a second key still counts as a `g`-prefix navigation.
const G_WINDOW: Duration = Duration::from_millis(1200);

/// The `g`-prefix state machine: idle, or "`g` pressed, awaiting the second key" with a deadline.
#[derive(Debug, Clone, Copy)]
pub enum GPrefix {
    /// No prefix pending.
    Idle,
    /// `g` was pressed at `since`; a category key within [`G_WINDOW`] navigates.
    Armed {
        /// When `g` was pressed.
        since: Instant,
    },
}

/// Installs the `g`-prefix state and returns its signal for the shell root to thread into
/// [`dispatch`].
#[must_use]
pub fn use_keyboard_dispatch() -> Signal<GPrefix> {
    use_signal(|| GPrefix::Idle)
}

/// The single keydown entry point, wired on the shell root.
pub fn dispatch(event: &KeyboardEvent, mut nav: NavState, mut gp: Signal<GPrefix>) {
    let key = event.key();

    if consume_g_prefix(event, &key, &mut nav, &mut gp) {
        return;
    }

    if key == Key::Escape {
        event.prevent_default();
        nav.close_overlay();
        return;
    }

    if primary_modifier(event.modifiers()) {
        dispatch_command(event, &mut nav);
    } else {
        dispatch_bare(event, &key, &mut nav, &mut gp);
    }
}

/// Resolves an armed `g`-prefix: disarms it, and if the second key (within the window) is bound to a
/// category, navigates there. Returns whether the key was consumed by navigation.
fn consume_g_prefix(event: &KeyboardEvent, key: &Key, nav: &mut NavState, gp: &mut Signal<GPrefix>) -> bool {
    let GPrefix::Armed { since } = *gp.peek() else {
        return false;
    };
    gp.set(GPrefix::Idle);
    if since.elapsed() > G_WINDOW {
        return false;
    }
    let Key::Character(second) = key else {
        return false;
    };
    let Some(destination) = go_destination(second) else {
        return false;
    };
    event.prevent_default();
    nav.go_to(destination);
    true
}

/// Handles primary-modifier chords (`⌘K`/`⌘F`/`⌘N`/`⌘1…9`; undo/redo are deferred to the History PR).
fn dispatch_command(event: &KeyboardEvent, nav: &mut NavState) {
    match event.key() {
        Key::Character(character) if character == "k" || character == "f" => {
            event.prevent_default();
            nav.overlay.set(Overlay::Palette);
        }
        Key::Character(character) if character == "n" => {
            event.prevent_default();
            tracing::debug!("new-record shortcut: context-aware creation lands with the editing PRs");
        }
        _ => {
            if let Some(n) = digit_1_to_9(event.code()) {
                event.prevent_default();
                nav.switch_tab(n);
            } else {
                tracing::debug!("unhandled command chord (undo/redo land with the History PR)");
            }
        }
    }
}

/// Handles bare keys that are shell-global: `?` opens help, `g` arms the navigation prefix.
fn dispatch_bare(event: &KeyboardEvent, key: &Key, nav: &mut NavState, gp: &mut Signal<GPrefix>) {
    let Key::Character(character) = key else {
        return;
    };
    match character.as_str() {
        "?" => {
            event.prevent_default();
            nav.overlay.set(Overlay::Help);
        }
        "g" => {
            event.prevent_default();
            gp.set(GPrefix::Armed { since: Instant::now() });
        }
        _ => {}
    }
}

/// `true` when the platform's primary modifier (`⌘` on macOS, `Ctrl` elsewhere) is held.
fn primary_modifier(modifiers: Modifiers) -> bool {
    if cfg!(target_os = "macos") {
        modifiers.meta()
    } else {
        modifiers.ctrl()
    }
}

/// The destination a `g`-prefix second key navigates to, if the key is bound to a category.
fn go_destination(second: &str) -> Option<Destination> {
    let key = second.chars().next()?;
    Category::from_nav_key(key).map(Destination::Category)
}

/// `Some(1..=9)` when the physical code is `Digit1`…`Digit9` (layout-independent `⌘1…9`).
fn digit_1_to_9(code: Code) -> Option<u8> {
    match code {
        Code::Digit1 => Some(1),
        Code::Digit2 => Some(2),
        Code::Digit3 => Some(3),
        Code::Digit4 => Some(4),
        Code::Digit5 => Some(5),
        Code::Digit6 => Some(6),
        Code::Digit7 => Some(7),
        Code::Digit8 => Some(8),
        Code::Digit9 => Some(9),
        _ => None,
    }
}
