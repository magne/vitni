//! The keyboard shortcut map: one source of truth for the dispatcher and the `?` help overlay.
//!
//! Framework-neutral (ADR 0008): a [`Chord`] describes a key + modifier *logically*; the renderer
//! matches its own framework key events against it and renders the overlay from [`shortcuts`].
//! Labels are Fluent message ids (ADR 0003), resolved by the renderer's chrome catalogue.
//!
//! The `g`-prefix navigation rows are *not* in [`shortcuts`]: they are generated from
//! [`Category::nav_key`] by [`navigation_shortcuts`], so the rail and the overlay share one source.

use crate::navigation::Category;

/// A modifier key, in the logical sense.
///
/// [`Self::Command`] is `⌘` on macOS and `Ctrl` elsewhere — the renderer resolves the platform; the
/// map never hardcodes a platform glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Modifier {
    /// No modifier.
    None,
    /// The primary command modifier (`⌘`/`Ctrl`).
    Command,
    /// Command plus Shift.
    CommandShift,
}

/// A logical key in a chord. Covers every key the shortcut spec uses; no framework key type leaks in.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Key {
    /// A letter key, stored lowercase (e.g. `Key::Char('k')`).
    Char(char),
    /// A single digit key.
    Digit(u8),
    /// `1…9` — switch to record tab N (a range, rendered as one overlay row).
    DigitRange,
    /// `?` help.
    Question,
    /// Escape.
    Escape,
    /// Enter / Return.
    Enter,
    /// Arrow up.
    ArrowUp,
    /// Arrow down.
    ArrowDown,
    /// Arrow left.
    ArrowLeft,
    /// Arrow right.
    ArrowRight,
    /// Home.
    Home,
    /// End.
    End,
    /// `[` previous.
    BracketLeft,
    /// `]` next.
    BracketRight,
}

/// A key chord: a modifier plus a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    /// The modifier (or [`Modifier::None`]).
    pub modifier: Modifier,
    /// The key.
    pub key: Key,
}

/// A shortcut's group in the `?` overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutGroup {
    /// Global shortcuts, active anywhere.
    Global,
    /// `g`-prefix navigation (press `g`, then a category key).
    Navigation,
    /// Within-screen shortcuts (list / tabs / facts).
    WithinScreen,
}

/// The action a shortcut invokes — a stable, framework-neutral handle the renderer dispatches on.
///
/// This covers the global and within-screen actions only; the `g`-prefix navigation actions are
/// generated per category (see [`navigation_shortcuts`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutAction {
    /// Open the command palette (`⌘K`).
    CommandPalette,
    /// New record, context-aware (`⌘N`).
    NewRecord,
    /// Find / filter (`⌘F`).
    Find,
    /// Undo (`⌘Z`).
    Undo,
    /// Redo (`⌘⇧Z`).
    Redo,
    /// Switch to record tab N (`⌘1…9`).
    SwitchRecordTab,
    /// Dock record tab N side-by-side with the active record (`⌘⇧1…9`).
    DockRecordTab,
    /// Toggle the shortcut help overlay (`?`).
    Help,
    /// Close / clear (`Esc`).
    Close,
    /// Move list selection up (`↑`).
    MoveUp,
    /// Move list selection down (`↓`).
    MoveDown,
    /// Open the selected record (`Enter`).
    Open,
    /// Previous record (`[`).
    PrevRecord,
    /// Next record (`]`).
    NextRecord,
    /// Previous detail tab (`←`).
    PrevTab,
    /// Next detail tab (`→`).
    NextTab,
    /// First detail tab (`Home`).
    FirstTab,
    /// Last detail tab (`End`).
    LastTab,
    /// Add a source to the focused fact (`s`).
    AddSource,
    /// Edit the focused fact (`e`).
    Edit,
    /// Quit the application (`⌘Q`).
    Quit,
    /// Close the active record tab (`⌘W`).
    CloseCurrentTab,
}

impl ShortcutAction {
    /// Every action, used to assert the map is exhaustive.
    #[must_use]
    pub const fn all() -> [Self; 22] {
        [
            Self::CommandPalette,
            Self::NewRecord,
            Self::Find,
            Self::Undo,
            Self::Redo,
            Self::SwitchRecordTab,
            Self::DockRecordTab,
            Self::Help,
            Self::Close,
            Self::MoveUp,
            Self::MoveDown,
            Self::Open,
            Self::PrevRecord,
            Self::NextRecord,
            Self::PrevTab,
            Self::NextTab,
            Self::FirstTab,
            Self::LastTab,
            Self::AddSource,
            Self::Edit,
            Self::Quit,
            Self::CloseCurrentTab,
        ]
    }
}

/// One shortcut: an action, its chord, its overlay group, and its Fluent label id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Shortcut {
    /// The action invoked.
    pub action: ShortcutAction,
    /// The chord that triggers it.
    pub chord: Chord,
    /// Which `?`-overlay group it belongs to.
    pub group: ShortcutGroup,
    /// The Fluent message id describing the shortcut (resolved by the renderer's chrome catalogue).
    pub label_id: &'static str,
}

/// Builds one shortcut entry.
const fn shortcut(
    action: ShortcutAction,
    modifier: Modifier,
    key: Key,
    group: ShortcutGroup,
    label_id: &'static str,
) -> Shortcut {
    Shortcut {
        action,
        chord: Chord { modifier, key },
        group,
        label_id,
    }
}

/// The complete global + within-screen shortcut map, in `?`-overlay order.
///
/// The `g`-prefix navigation rows are *not* here — use [`navigation_shortcuts`] for those, so the
/// rail and overlay share one source.
#[must_use]
pub fn shortcuts() -> Vec<Shortcut> {
    use Key::{
        ArrowDown, ArrowLeft, ArrowRight, ArrowUp, BracketLeft, BracketRight, Char, DigitRange, End, Enter, Escape,
        Home, Question,
    };
    use Modifier::{Command, CommandShift, None as NoMod};
    use ShortcutAction::{
        AddSource, Close, CloseCurrentTab, CommandPalette, DockRecordTab, Edit, Find, FirstTab, Help, LastTab,
        MoveDown, MoveUp, NewRecord, NextRecord, NextTab, Open, PrevRecord, PrevTab, Quit, Redo, SwitchRecordTab, Undo,
    };
    use ShortcutGroup::{Global, WithinScreen};
    vec![
        shortcut(CommandPalette, Command, Char('k'), Global, "sc-command-palette"),
        shortcut(NewRecord, Command, Char('n'), Global, "sc-new-record"),
        shortcut(Find, Command, Char('f'), Global, "sc-find"),
        shortcut(Undo, Command, Char('z'), Global, "sc-undo"),
        shortcut(Redo, CommandShift, Char('z'), Global, "sc-redo"),
        shortcut(SwitchRecordTab, Command, DigitRange, Global, "sc-switch-tab"),
        shortcut(DockRecordTab, CommandShift, DigitRange, Global, "sc-dock-tab"),
        shortcut(Help, NoMod, Question, Global, "sc-help"),
        shortcut(Close, NoMod, Escape, Global, "sc-close"),
        shortcut(Quit, Command, Char('q'), Global, "sc-quit"),
        shortcut(CloseCurrentTab, Command, Char('w'), Global, "sc-close-tab"),
        shortcut(MoveUp, NoMod, ArrowUp, WithinScreen, "sc-move-up"),
        shortcut(MoveDown, NoMod, ArrowDown, WithinScreen, "sc-move-down"),
        shortcut(Open, NoMod, Enter, WithinScreen, "sc-open"),
        shortcut(PrevRecord, NoMod, BracketLeft, WithinScreen, "sc-prev-record"),
        shortcut(NextRecord, NoMod, BracketRight, WithinScreen, "sc-next-record"),
        shortcut(PrevTab, NoMod, ArrowLeft, WithinScreen, "sc-prev-tab"),
        shortcut(NextTab, NoMod, ArrowRight, WithinScreen, "sc-next-tab"),
        shortcut(FirstTab, NoMod, Home, WithinScreen, "sc-first-tab"),
        shortcut(LastTab, NoMod, End, WithinScreen, "sc-last-tab"),
        shortcut(AddSource, NoMod, Char('s'), WithinScreen, "sc-add-source"),
        shortcut(Edit, NoMod, Char('e'), WithinScreen, "sc-edit"),
    ]
}

/// A `g`-prefix navigation row for the help overlay: the second key, the target, and its label id.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct NavShortcut {
    /// The category navigated to.
    pub category: Category,
    /// The second key pressed after `g`.
    pub key: char,
    /// The Fluent label id (reuses the rail label id, e.g. `nav-people`).
    pub label_id: &'static str,
}

/// The `g`-prefix navigation rows, in rail order, for every category that has a nav key.
#[must_use]
pub fn navigation_shortcuts() -> Vec<NavShortcut> {
    let mut rows = Vec::new();
    for category in Category::all() {
        if let Some(key) = category.nav_key() {
            rows.push(NavShortcut {
                category,
                key,
                label_id: category.label_id(),
            });
        }
    }
    rows
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{Chord, Modifier, ShortcutAction, ShortcutGroup, navigation_shortcuts, shortcuts};
    use crate::navigation::Category;

    #[test]
    fn no_duplicate_chords() {
        let chords: Vec<Chord> = shortcuts().iter().map(|entry| entry.chord).collect();
        for (index, chord) in chords.iter().enumerate() {
            assert_eq!(
                chords.iter().filter(|other| *other == chord).count(),
                1,
                "duplicate chord at {index}"
            );
        }
    }

    #[test]
    fn every_action_appears_once() {
        let map = shortcuts();
        for action in ShortcutAction::all() {
            assert_eq!(
                map.iter().filter(|entry| entry.action == action).count(),
                1,
                "action {action:?} not unique"
            );
        }
        assert_eq!(map.len(), ShortcutAction::all().len());
    }

    #[test]
    fn group_counts_match_spec() {
        let map = shortcuts();
        let global = map.iter().filter(|entry| entry.group == ShortcutGroup::Global).count();
        let within = map
            .iter()
            .filter(|entry| entry.group == ShortcutGroup::WithinScreen)
            .count();
        assert_eq!(global, 11);
        assert_eq!(within, 11);
    }

    #[test]
    fn quit_and_close_tab_are_global_command_chords() {
        let map = shortcuts();
        let quit = map
            .iter()
            .find(|entry| entry.action == ShortcutAction::Quit)
            .expect("quit present");
        let close_tab = map
            .iter()
            .find(|entry| entry.action == ShortcutAction::CloseCurrentTab)
            .expect("close-current-tab present");
        assert_eq!(quit.group, ShortcutGroup::Global);
        assert_eq!(quit.chord.modifier, Modifier::Command);
        assert_eq!(quit.label_id, "sc-quit");
        assert_eq!(close_tab.group, ShortcutGroup::Global);
        assert_eq!(close_tab.chord.modifier, Modifier::Command);
        assert_eq!(close_tab.label_id, "sc-close-tab");
    }

    #[test]
    fn undo_redo_modifiers_differ() {
        let map = shortcuts();
        let undo = map
            .iter()
            .find(|entry| entry.action == ShortcutAction::Undo)
            .expect("undo present");
        let redo = map
            .iter()
            .find(|entry| entry.action == ShortcutAction::Redo)
            .expect("redo present");
        assert_eq!(undo.chord.modifier, Modifier::Command);
        assert_eq!(redo.chord.modifier, Modifier::CommandShift);
    }

    #[test]
    fn navigation_shortcuts_match_keyed_categories() {
        let rows = navigation_shortcuts();
        assert_eq!(rows.len(), 11);
        for row in &rows {
            assert_eq!(Some(row.key), row.category.nav_key());
            assert_eq!(row.label_id, row.category.label_id());
        }
        let keys: BTreeSet<char> = rows.iter().map(|row| row.key).collect();
        let expected: BTreeSet<char> = "dpfelscrmnt".chars().collect();
        assert_eq!(keys, expected);
        assert!(
            !rows
                .iter()
                .any(|row| matches!(row.category, Category::DnaTests | Category::DnaMatches))
        );
    }
}
