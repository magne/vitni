//! The central keyboard dispatcher.
//!
//! One `onkeydown` on the shell root interprets the resolved shortcut map
//! (`genealogy_ui::resolved_shortcuts`, ADR 0030) into shell actions: open the palette, toggle help,
//! navigate via the `g`-prefix, switch record tabs (`⌘1…9`), dock a record tab side-by-side
//! (`⌘⇧1…9`), step through records (`[`/`]`), undo (`⌘Z`), and close overlays.
//! The key→action decision is the pure [`shell_intent`] (unit-tested exhaustively over the *default*
//! map — the regression net a rebind must not break); [`dispatch`] is a thin interpreter that applies
//! the resulting [`ShellIntent`] to the shell state. The primary modifier is `⌘` on macOS and `Ctrl`
//! elsewhere. Within-screen keys owned by a focused widget (↑/↓, Enter, ←/→, Home/End) and the
//! `g`-prefix navigation keys are not rebindable (ADR 0030 §2) and stay hardcoded here.

use std::time::{Duration, Instant};

use dioxus::prelude::*;
use genealogy_ui::{Category, Destination, RecordRef, Shortcut, ShortcutAction, ShortcutGroup};

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
    /// Dismiss the topmost dismissable layer — the close/quit confirm if one is armed, otherwise any
    /// open overlay (`Esc`).
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
    /// Quit the application (`⌘Q`).
    Quit,
    /// Close the active record tab (`⌘W`).
    CloseCurrentTab,
}

/// Installs the `g`-prefix state and returns its signal for the shell root to thread into
/// [`dispatch`].
#[must_use]
pub fn use_keyboard_dispatch() -> Signal<GPrefix> {
    use_signal(|| GPrefix::Idle)
}

/// Maps one keydown to a [`ShellIntent`], if it is a shell-global chord. Pure over plain data
/// (`dioxus` `Key`/`Modifiers`/`Code` carry no state) so the full matrix is unit-testable; `primary`
/// is whether the platform's primary modifier is held (resolved by the caller). `resolved` is the
/// live resolved shortcut map (`genealogy_ui::resolved_shortcuts`, ADR 0030) — every `Global` action
/// (except `Close`, whose `Esc` binding is a fixed platform convention, and the digit-range actions,
/// matched by physical code below) is a lookup against it, so a rebind takes effect here for free.
/// The `g`-prefix's *second* key is not decided here — [`dispatch`] resolves that statefully first.
#[must_use]
pub fn shell_intent(
    key: &Key,
    modifiers: Modifiers,
    code: Code,
    primary: bool,
    resolved: &[Shortcut],
) -> Option<ShellIntent> {
    if *key == Key::Escape {
        return Some(ShellIntent::CloseOverlay);
    }
    // Not rebindable (ADR 0030 §2): the `g`-prefix arm and the bracket record-step keys are bare,
    // fixed chords, never looked up against the resolved map.
    if !primary && let Key::Character(character) = key {
        match character.as_str() {
            "g" => return Some(ShellIntent::ArmGPrefix),
            "[" => return Some(ShellIntent::StepRecord(-1)),
            "]" => return Some(ShellIntent::StepRecord(1)),
            _ => {}
        }
    }
    // `⌘1…9`/`⌘⇧1…9`: layout-independent by physical code, but the *modifier* each requires is
    // still the resolved (rebindable) one — `Key::DigitRange` cannot be typed literally, so it is
    // matched here rather than through the generic lookup below.
    if let Some(digit) = digit_1_to_9(code) {
        let held = event_modifier(modifiers, primary);
        if Some(held) == resolved_modifier(resolved, ShortcutAction::DockRecordTab) {
            return Some(ShellIntent::DockRecordTab(digit));
        }
        if Some(held) == resolved_modifier(resolved, ShortcutAction::SwitchRecordTab) {
            return Some(ShellIntent::SwitchRecordTab(digit));
        }
        return None;
    }
    let chord_key = shortcut_key(key)?;
    let mut modifier = event_modifier(modifiers, primary);
    if chord_key == genealogy_ui::Key::Question {
        // `?` is typed with Shift on every common layout (US `shift+/`, Norwegian `shift++`), so the
        // event always reports it. Shift here is how the character was *produced*, not a modifier of
        // it — keeping it would mean the `?` help chord (declared with no modifiers) never matches.
        modifier.shift = false;
    }
    let chord = genealogy_ui::Chord {
        modifier,
        key: chord_key,
    };
    let action = resolved
        .iter()
        .find(|entry| entry.group == ShortcutGroup::Global && entry.chord == chord)?
        .action;
    shell_intent_for_action(action)
}

/// The [`ShellIntent`] a rebindable ([`ShortcutGroup::Global`]) action produces. `None` for an action
/// this dispatcher does not reach through the lookup (every non-`Global` action, and `Close`/the
/// digit-range actions, which are matched earlier) — never actually returned in practice, but the
/// match stays total over [`ShortcutAction`] so a newly-added action cannot be forgotten silently.
fn shell_intent_for_action(action: ShortcutAction) -> Option<ShellIntent> {
    match action {
        ShortcutAction::CommandPalette | ShortcutAction::Find => Some(ShellIntent::OpenPalette),
        ShortcutAction::NewRecord => Some(ShellIntent::NewRecord),
        ShortcutAction::Undo => Some(ShellIntent::Undo),
        ShortcutAction::Redo => Some(ShellIntent::Redo),
        ShortcutAction::Help => Some(ShellIntent::Help),
        ShortcutAction::Close => Some(ShellIntent::CloseOverlay),
        ShortcutAction::Quit => Some(ShellIntent::Quit),
        ShortcutAction::CloseCurrentTab => Some(ShellIntent::CloseCurrentTab),
        ShortcutAction::SwitchRecordTab
        | ShortcutAction::DockRecordTab
        | ShortcutAction::MoveUp
        | ShortcutAction::MoveDown
        | ShortcutAction::Open
        | ShortcutAction::PrevRecord
        | ShortcutAction::NextRecord
        | ShortcutAction::PrevTab
        | ShortcutAction::NextTab
        | ShortcutAction::FirstTab
        | ShortcutAction::LastTab
        | ShortcutAction::AddSource
        | ShortcutAction::Edit => None,
    }
}

/// The modifier the resolved map requires for `action`, or `None` if `action` is absent (never
/// happens — [`genealogy_ui::resolved_shortcuts`] always returns every action, at its default chord
/// if no override was accepted).
fn resolved_modifier(resolved: &[Shortcut], action: ShortcutAction) -> Option<genealogy_ui::Modifier> {
    resolved
        .iter()
        .find(|entry| entry.action == action)
        .map(|entry| entry.chord.modifier)
}

/// The event's modifier as a [`genealogy_ui::Modifier`]: `command` is the platform primary modifier
/// (resolved by the caller), `shift`/`alt` read straight off the `dioxus` event.
fn event_modifier(modifiers: Modifiers, primary: bool) -> genealogy_ui::Modifier {
    genealogy_ui::Modifier {
        command: primary,
        shift: modifiers.shift(),
        alt: modifiers.alt(),
    }
}

/// Maps a `dioxus` key event to the [`genealogy_ui::Key`] a chord lookup matches against: `?` and
/// single ASCII letters only (every `Global` action's key is one of those). `None` for anything else
/// (arrows, Enter, digits, punctuation) — those are either matched earlier (brackets, digits) or
/// belong to a non-rebindable within-screen/`g`-prefix chord this dispatcher does not reach here.
fn shortcut_key(key: &Key) -> Option<genealogy_ui::Key> {
    let Key::Character(character) = key else {
        return None;
    };
    if character == "?" {
        return Some(genealogy_ui::Key::Question);
    }
    let mut chars = character.chars();
    let only = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    only.is_ascii_alphabetic()
        .then(|| genealogy_ui::Key::Char(only.to_ascii_lowercase()))
}

/// The single keydown entry point, wired on the shell root. `resolved` is the live resolved shortcut
/// map (ADR 0030) the caller computed from the current `[shortcuts]` overrides.
pub fn dispatch(
    event: &KeyboardEvent,
    mut nav: NavState,
    mut gp: Signal<GPrefix>,
    notices: &ShellNotices,
    resolved: &[Shortcut],
) {
    let key = event.key();
    if consume_g_prefix(event, &key, &mut nav, &mut gp) {
        return;
    }
    let primary = primary_modifier(event.modifiers());
    let Some(intent) = shell_intent(&key, event.modifiers(), event.code(), primary, resolved) else {
        return;
    };
    event.prevent_default();
    match intent {
        ShellIntent::CloseOverlay => nav.dismiss_topmost(),
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
        ShellIntent::Quit => nav.request_quit(),
        ShellIntent::CloseCurrentTab => {
            let active = *nav.active_record.peek();
            if let Some(index) = active {
                nav.request_close_tab(index);
            }
        }
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
    use std::collections::BTreeMap;

    use dioxus::prelude::{Code, Key, Modifiers};
    use genealogy_ui::{ShortcutAction, resolved_shortcuts, shortcuts};

    use super::{ShellIntent, shell_intent};

    fn character(c: &str) -> Key {
        Key::Character(c.to_owned())
    }

    /// The default map (no overrides) — the regression net every existing chord must keep matching.
    fn defaults() -> Vec<genealogy_ui::Shortcut> {
        shortcuts()
    }

    #[test]
    fn escape_closes_regardless_of_modifier() {
        assert_eq!(
            shell_intent(&Key::Escape, Modifiers::empty(), Code::Escape, false, &defaults()),
            Some(ShellIntent::CloseOverlay)
        );
    }

    #[test]
    fn primary_letter_chords_map_to_their_actions() {
        assert_eq!(
            shell_intent(&character("k"), Modifiers::empty(), Code::KeyK, true, &defaults()),
            Some(ShellIntent::OpenPalette)
        );
        assert_eq!(
            shell_intent(&character("f"), Modifiers::empty(), Code::KeyF, true, &defaults()),
            Some(ShellIntent::OpenPalette)
        );
        assert_eq!(
            shell_intent(&character("n"), Modifiers::empty(), Code::KeyN, true, &defaults()),
            Some(ShellIntent::NewRecord)
        );
    }

    #[test]
    fn undo_and_redo_differ_by_shift() {
        assert_eq!(
            shell_intent(&character("z"), Modifiers::empty(), Code::KeyZ, true, &defaults()),
            Some(ShellIntent::Undo)
        );
        assert_eq!(
            shell_intent(&character("z"), Modifiers::SHIFT, Code::KeyZ, true, &defaults()),
            Some(ShellIntent::Redo)
        );
    }

    #[test]
    fn undo_needs_the_primary_modifier() {
        // A bare `z` is not a shell chord (it types into a field / is ignored).
        assert_eq!(
            shell_intent(&character("z"), Modifiers::empty(), Code::KeyZ, false, &defaults()),
            None
        );
    }

    #[test]
    fn digits_switch_tabs_via_the_physical_code() {
        assert_eq!(
            shell_intent(&character("1"), Modifiers::empty(), Code::Digit1, true, &defaults()),
            Some(ShellIntent::SwitchRecordTab(1))
        );
        assert_eq!(
            shell_intent(&character("9"), Modifiers::empty(), Code::Digit9, true, &defaults()),
            Some(ShellIntent::SwitchRecordTab(9))
        );
        // `⌘0` is not a tab switch.
        assert_eq!(
            shell_intent(&character("0"), Modifiers::empty(), Code::Digit0, true, &defaults()),
            None
        );
    }

    #[test]
    fn shift_digit_docks_while_bare_digit_switches() {
        assert_eq!(
            shell_intent(&character("3"), Modifiers::SHIFT, Code::Digit3, true, &defaults()),
            Some(ShellIntent::DockRecordTab(3))
        );
        assert_eq!(
            shell_intent(&character("3"), Modifiers::empty(), Code::Digit3, true, &defaults()),
            Some(ShellIntent::SwitchRecordTab(3))
        );
    }

    #[test]
    fn shift_z_is_still_redo_not_a_dock() {
        assert_eq!(
            shell_intent(&character("z"), Modifiers::SHIFT, Code::KeyZ, true, &defaults()),
            Some(ShellIntent::Redo)
        );
    }

    #[test]
    fn brackets_step_records_only_when_bare() {
        assert_eq!(
            shell_intent(
                &character("["),
                Modifiers::empty(),
                Code::BracketLeft,
                false,
                &defaults()
            ),
            Some(ShellIntent::StepRecord(-1))
        );
        assert_eq!(
            shell_intent(
                &character("]"),
                Modifiers::empty(),
                Code::BracketRight,
                false,
                &defaults()
            ),
            Some(ShellIntent::StepRecord(1))
        );
        // With the primary modifier held, `[`/`]` are not record steps.
        assert_eq!(
            shell_intent(
                &character("["),
                Modifiers::empty(),
                Code::BracketLeft,
                true,
                &defaults()
            ),
            None
        );
        assert_eq!(
            shell_intent(
                &character("]"),
                Modifiers::empty(),
                Code::BracketRight,
                true,
                &defaults()
            ),
            None
        );
    }

    #[test]
    fn bare_help_and_gprefix() {
        assert_eq!(
            shell_intent(&character("?"), Modifiers::empty(), Code::Slash, false, &defaults()),
            Some(ShellIntent::Help)
        );
        assert_eq!(
            shell_intent(&character("g"), Modifiers::empty(), Code::KeyG, false, &defaults()),
            Some(ShellIntent::ArmGPrefix)
        );
    }

    /// The test above presses `?` with no modifiers, which no keyboard can actually produce: `?` is
    /// `shift+/` on US layouts and `shift++` on Norwegian ones, so a real event always reports Shift.
    /// Matching that against the no-modifier help chord is what makes the overlay reachable at all.
    #[test]
    fn help_matches_with_the_shift_that_types_the_question_mark() {
        assert_eq!(
            shell_intent(&character("?"), Modifiers::SHIFT, Code::Slash, false, &defaults()),
            Some(ShellIntent::Help)
        );
    }

    #[test]
    fn unbound_keys_are_ignored() {
        assert_eq!(
            shell_intent(&character("y"), Modifiers::empty(), Code::KeyY, false, &defaults()),
            None
        );
        assert_eq!(
            shell_intent(&character("y"), Modifiers::empty(), Code::KeyY, true, &defaults()),
            None
        );
    }

    #[test]
    fn primary_q_quits_and_primary_w_closes_the_current_tab() {
        assert_eq!(
            shell_intent(&character("q"), Modifiers::empty(), Code::KeyQ, true, &defaults()),
            Some(ShellIntent::Quit)
        );
        assert_eq!(
            shell_intent(&character("w"), Modifiers::empty(), Code::KeyW, true, &defaults()),
            Some(ShellIntent::CloseCurrentTab)
        );
    }

    #[test]
    fn bare_q_and_w_are_ignored_so_they_still_type_into_fields() {
        assert_eq!(
            shell_intent(&character("q"), Modifiers::empty(), Code::KeyQ, false, &defaults()),
            None
        );
        assert_eq!(
            shell_intent(&character("w"), Modifiers::empty(), Code::KeyW, false, &defaults()),
            None
        );
    }

    #[test]
    fn a_rebound_chord_changes_what_shell_intent_returns() {
        // Rebind quit from `mod+q` to `mod+j`; `mod+q` must stop firing and `mod+j` must now fire.
        let overrides = BTreeMap::from([("quit".to_owned(), "mod+j".to_owned())]);
        let (resolved, errors) = resolved_shortcuts(&overrides);
        assert!(errors.is_empty());
        assert_eq!(
            shell_intent(&character("q"), Modifiers::empty(), Code::KeyQ, true, &resolved),
            None,
            "the old chord no longer fires once rebound"
        );
        assert_eq!(
            shell_intent(&character("j"), Modifiers::empty(), Code::KeyJ, true, &resolved),
            Some(ShellIntent::Quit),
            "the new chord fires the same action"
        );
    }

    #[test]
    fn a_rebound_action_still_appears_in_the_action_set() {
        // Sanity: the rebind test above targets a real, currently-Global action.
        assert!(shortcuts().iter().any(|entry| entry.action == ShortcutAction::Quit));
    }
}
