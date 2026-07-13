//! The central keyboard dispatcher.
//!
//! One `onkeydown` on the shell root interprets the framework-neutral shortcut map
//! (`genealogy_ui::shortcuts`) into shell actions: open the palette, toggle help, navigate via the
//! `g`-prefix, switch record tabs (`⌘1…9`), dock a record tab side-by-side (`⌘⇧1…9`), step through
//! records (`[`/`]`), undo (`⌘Z`), and close overlays.
//! The key→action decision is the pure [`shell_intent`] (unit-tested exhaustively); [`dispatch`] is a
//! thin interpreter that applies the resulting [`ShellIntent`] to the shell state. The primary
//! modifier is `⌘` on macOS and `Ctrl` elsewhere. Within-screen keys owned by a focused widget
//! (↑/↓, Enter, ←/→, Home/End) are not handled here.

use std::time::{Duration, Instant};

use dioxus::prelude::*;
use genealogy_ui::{Category, Destination, RecordRef};

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

/// The already-localized notice strings the dispatcher shows for a shortcut outcome.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellNotices {
    /// Shown by `⌘Z` when no record is open on its own screen.
    pub nothing_to_undo: String,
    /// Shown by `⌘⇧Z` (redo is unavailable — the log is append-only).
    pub redo_unavailable: String,
}

/// A shell-global action a key chord maps to — the interpreted result of one keydown, independent of
/// the `g`-prefix timing (which [`dispatch`] resolves statefully before consulting this).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShellIntent {
    /// Close any open overlay (`Esc`).
    CloseOverlay,
    /// Open the command palette (`⌘K`/`⌘F`).
    OpenPalette,
    /// Create a new record on the active screen (`⌘N`).
    NewRecord,
    /// Switch to the 1-based record tab (`⌘1…9`).
    SwitchRecordTab(u8),
    /// Dock the 1-based record tab side-by-side with the active record (`⌘⇧1…9`).
    DockRecordTab(u8),
    /// Undo the active record's newest undoable assertion (`⌘Z`).
    Undo,
    /// Redo (`⌘⇧Z`) — unavailable; shows the append-only explanation.
    Redo,
    /// Toggle the shortcut help overlay (`?`).
    Help,
    /// Arm the `g`-prefix navigation.
    ArmGPrefix,
    /// Step to the previous/next record (`[` = `-1`, `]` = `+1`).
    StepRecord(i8),
}

/// Installs the `g`-prefix state and returns its signal for the shell root to thread into
/// [`dispatch`].
#[must_use]
pub fn use_keyboard_dispatch() -> Signal<GPrefix> {
    use_signal(|| GPrefix::Idle)
}

/// Maps one keydown to a [`ShellIntent`], if it is a shell-global chord. Pure over plain data
/// (`dioxus` `Key`/`Modifiers`/`Code` carry no state) so the full matrix is unit-testable; `primary`
/// is whether the platform's primary modifier is held (resolved by the caller). The `g`-prefix's
/// *second* key is not decided here — [`dispatch`] resolves that statefully first.
#[must_use]
pub fn shell_intent(key: &Key, modifiers: Modifiers, code: Code, primary: bool) -> Option<ShellIntent> {
    if *key == Key::Escape {
        return Some(ShellIntent::CloseOverlay);
    }
    if primary {
        return match key {
            Key::Character(character) if character == "k" || character == "f" => Some(ShellIntent::OpenPalette),
            Key::Character(character) if character == "n" => Some(ShellIntent::NewRecord),
            Key::Character(character) if character == "z" => {
                if modifiers.shift() {
                    Some(ShellIntent::Redo)
                } else {
                    Some(ShellIntent::Undo)
                }
            }
            _ => digit_1_to_9(code).map(|n| {
                if modifiers.shift() {
                    ShellIntent::DockRecordTab(n)
                } else {
                    ShellIntent::SwitchRecordTab(n)
                }
            }),
        };
    }
    let Key::Character(character) = key else {
        return None;
    };
    match character.as_str() {
        "?" => Some(ShellIntent::Help),
        "g" => Some(ShellIntent::ArmGPrefix),
        "[" => Some(ShellIntent::StepRecord(-1)),
        "]" => Some(ShellIntent::StepRecord(1)),
        _ => None,
    }
}

/// The single keydown entry point, wired on the shell root.
pub fn dispatch(event: &KeyboardEvent, mut nav: NavState, mut gp: Signal<GPrefix>, notices: &ShellNotices) {
    let key = event.key();
    if consume_g_prefix(event, &key, &mut nav, &mut gp) {
        return;
    }
    let primary = primary_modifier(event.modifiers());
    let Some(intent) = shell_intent(&key, event.modifiers(), event.code(), primary) else {
        return;
    };
    event.prevent_default();
    match intent {
        ShellIntent::CloseOverlay => nav.close_overlay(),
        ShellIntent::OpenPalette => nav.overlay.set(Overlay::Palette),
        ShellIntent::NewRecord => nav.request_new(),
        ShellIntent::SwitchRecordTab(n) => nav.switch_record(n),
        ShellIntent::DockRecordTab(n) => nav.dock_record_tab(n),
        ShellIntent::Undo => {
            if undo_targets(*nav.active.peek(), nav.active_record_ref().as_ref()) {
                nav.request_undo();
            } else {
                nav.notify(notices.nothing_to_undo.clone());
            }
        }
        ShellIntent::Redo => nav.notify(notices.redo_unavailable.clone()),
        ShellIntent::Help => nav.overlay.set(Overlay::Help),
        ShellIntent::ArmGPrefix => gp.set(GPrefix::Armed { since: Instant::now() }),
        ShellIntent::StepRecord(delta) => nav.step_record(delta),
    }
}

/// Whether `⌘Z` can target the active record: a record is open and the work area is showing that
/// record's own category, so undo acts on what the operator is looking at.
#[must_use]
fn undo_targets(active: Destination, active_record: Option<&RecordRef>) -> bool {
    active_record.is_some_and(|record| active == Destination::Category(record.category))
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

#[cfg(test)]
mod tests {
    use super::{ShellIntent, shell_intent};
    use dioxus::prelude::{Code, Key, Modifiers};

    fn character(c: &str) -> Key {
        Key::Character(c.to_owned())
    }

    #[test]
    fn escape_closes_regardless_of_modifier() {
        assert_eq!(
            shell_intent(&Key::Escape, Modifiers::empty(), Code::Escape, false),
            Some(ShellIntent::CloseOverlay)
        );
    }

    #[test]
    fn primary_letter_chords_map_to_their_actions() {
        assert_eq!(
            shell_intent(&character("k"), Modifiers::empty(), Code::KeyK, true),
            Some(ShellIntent::OpenPalette)
        );
        assert_eq!(
            shell_intent(&character("f"), Modifiers::empty(), Code::KeyF, true),
            Some(ShellIntent::OpenPalette)
        );
        assert_eq!(
            shell_intent(&character("n"), Modifiers::empty(), Code::KeyN, true),
            Some(ShellIntent::NewRecord)
        );
    }

    #[test]
    fn undo_and_redo_differ_by_shift() {
        assert_eq!(
            shell_intent(&character("z"), Modifiers::empty(), Code::KeyZ, true),
            Some(ShellIntent::Undo)
        );
        assert_eq!(
            shell_intent(&character("z"), Modifiers::SHIFT, Code::KeyZ, true),
            Some(ShellIntent::Redo)
        );
    }

    #[test]
    fn undo_needs_the_primary_modifier() {
        // A bare `z` is not a shell chord (it types into a field / is ignored).
        assert_eq!(
            shell_intent(&character("z"), Modifiers::empty(), Code::KeyZ, false),
            None
        );
    }

    #[test]
    fn digits_switch_tabs_via_the_physical_code() {
        assert_eq!(
            shell_intent(&character("1"), Modifiers::empty(), Code::Digit1, true),
            Some(ShellIntent::SwitchRecordTab(1))
        );
        assert_eq!(
            shell_intent(&character("9"), Modifiers::empty(), Code::Digit9, true),
            Some(ShellIntent::SwitchRecordTab(9))
        );
        // `⌘0` is not a tab switch.
        assert_eq!(
            shell_intent(&character("0"), Modifiers::empty(), Code::Digit0, true),
            None
        );
    }

    #[test]
    fn shift_digit_docks_while_bare_digit_switches() {
        assert_eq!(
            shell_intent(&character("3"), Modifiers::SHIFT, Code::Digit3, true),
            Some(ShellIntent::DockRecordTab(3))
        );
        assert_eq!(
            shell_intent(&character("3"), Modifiers::empty(), Code::Digit3, true),
            Some(ShellIntent::SwitchRecordTab(3))
        );
    }

    #[test]
    fn shift_z_is_still_redo_not_a_dock() {
        assert_eq!(
            shell_intent(&character("z"), Modifiers::SHIFT, Code::KeyZ, true),
            Some(ShellIntent::Redo)
        );
    }

    #[test]
    fn brackets_step_records_only_when_bare() {
        assert_eq!(
            shell_intent(&character("["), Modifiers::empty(), Code::BracketLeft, false),
            Some(ShellIntent::StepRecord(-1))
        );
        assert_eq!(
            shell_intent(&character("]"), Modifiers::empty(), Code::BracketRight, false),
            Some(ShellIntent::StepRecord(1))
        );
        // With the primary modifier held, `[`/`]` are not record steps.
        assert_eq!(
            shell_intent(&character("["), Modifiers::empty(), Code::BracketLeft, true),
            None
        );
        assert_eq!(
            shell_intent(&character("]"), Modifiers::empty(), Code::BracketRight, true),
            None
        );
    }

    #[test]
    fn bare_help_and_gprefix() {
        assert_eq!(
            shell_intent(&character("?"), Modifiers::empty(), Code::Slash, false),
            Some(ShellIntent::Help)
        );
        assert_eq!(
            shell_intent(&character("g"), Modifiers::empty(), Code::KeyG, false),
            Some(ShellIntent::ArmGPrefix)
        );
    }

    #[test]
    fn unbound_keys_are_ignored() {
        assert_eq!(
            shell_intent(&character("q"), Modifiers::empty(), Code::KeyQ, false),
            None
        );
        assert_eq!(
            shell_intent(&character("q"), Modifiers::empty(), Code::KeyQ, true),
            None
        );
    }
}
