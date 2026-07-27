//! The keyboard shortcut map: one source of truth for the dispatcher and the `?` help overlay
//! (ADR 0030).
//!
//! Framework-neutral (ADR 0008): a [`Chord`] describes a key + modifier *logically*; the renderer
//! matches its own framework key events against it and renders the overlay from the resolved map.
//! Labels are Fluent message ids (ADR 0003), resolved by the renderer's chrome catalogue.
//!
//! [`shortcuts`] is the built-in default map. A workspace may override any [`ShortcutGroup::Global`]
//! binding (client-scope `[shortcuts]` config, ADR 0030 §3); [`resolved_shortcuts`] merges the
//! overrides in and is the **only** map the dispatcher and the overlay read, so the two can no longer
//! drift apart the way the pre-ADR-0030 hardcoded dispatcher and this map once did.
//!
//! The `g`-prefix navigation rows are *not* in [`shortcuts`]: they are generated from
//! [`Category::nav_key`] by [`navigation_shortcuts`], so the rail and the overlay share one source.
//! They are not rebindable (ADR 0030 §2).

use std::collections::BTreeMap;
use std::fmt;
use std::str::FromStr;

use crate::navigation::Category;

/// A modifier combination, in the logical sense: independent flags, so `Alt` composes freely with the
/// primary modifier and Shift (ADR 0030 §5 — replaces the prior three-variant enum, which could not
/// express `Alt` or an independent Shift).
///
/// [`Self::command`] is `⌘` on macOS and `Ctrl` elsewhere — the renderer resolves the platform; the
/// map never hardcodes a platform glyph.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub struct Modifier {
    /// The primary command modifier (`⌘`/`Ctrl`) is held.
    pub command: bool,
    /// Shift is held.
    pub shift: bool,
    /// Alt/Option is held.
    pub alt: bool,
}

impl Modifier {
    /// No modifier.
    pub const NONE: Self = Self {
        command: false,
        shift: false,
        alt: false,
    };
    /// The primary command modifier alone.
    pub const COMMAND: Self = Self {
        command: true,
        shift: false,
        alt: false,
    };
    /// Command plus Shift.
    pub const COMMAND_SHIFT: Self = Self {
        command: true,
        shift: true,
        alt: false,
    };
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

/// The canonical token for one key, used by [`Chord`]'s `FromStr`/`Display` (ADR 0030 §4).
fn key_token(key: Key) -> String {
    match key {
        Key::Char(character) => character.to_string(),
        Key::Digit(digit) => digit.to_string(),
        Key::DigitRange => "digit-range".to_owned(),
        Key::Question => "?".to_owned(),
        Key::Escape => "esc".to_owned(),
        Key::Enter => "enter".to_owned(),
        Key::ArrowUp => "up".to_owned(),
        Key::ArrowDown => "down".to_owned(),
        Key::ArrowLeft => "left".to_owned(),
        Key::ArrowRight => "right".to_owned(),
        Key::Home => "home".to_owned(),
        Key::End => "end".to_owned(),
        Key::BracketLeft => "[".to_owned(),
        Key::BracketRight => "]".to_owned(),
    }
}

/// Parses one key token, the inverse of [`key_token`]. `None` for anything not a recognized keyword
/// and not a single ASCII letter/digit.
fn key_from_token(token: &str) -> Option<Key> {
    match token {
        "?" => return Some(Key::Question),
        "esc" => return Some(Key::Escape),
        "enter" => return Some(Key::Enter),
        "up" => return Some(Key::ArrowUp),
        "down" => return Some(Key::ArrowDown),
        "left" => return Some(Key::ArrowLeft),
        "right" => return Some(Key::ArrowRight),
        "home" => return Some(Key::Home),
        "end" => return Some(Key::End),
        "[" => return Some(Key::BracketLeft),
        "]" => return Some(Key::BracketRight),
        "digit-range" => return Some(Key::DigitRange),
        _ => {}
    }
    let mut chars = token.chars();
    let only = chars.next()?;
    if chars.next().is_some() {
        return None;
    }
    if let Some(digit) = only.to_digit(10) {
        return u8::try_from(digit).ok().map(Key::Digit);
    }
    only.is_ascii_alphabetic().then(|| Key::Char(only.to_ascii_lowercase()))
}

/// A key chord: a modifier plus a key.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Chord {
    /// The modifier (or [`Modifier::NONE`]).
    pub modifier: Modifier,
    /// The key.
    pub key: Key,
}

/// A [`Chord`] string failed to parse (ADR 0030 §4): surfaced as a typed, user-visible error next to
/// the offending binding — never a silent drop.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ChordParseError {
    /// The chord string was empty.
    #[error("a shortcut cannot be empty")]
    Empty,
    /// A token was neither a recognized modifier (`mod`/`shift`/`alt`) nor a recognized key.
    #[error("'{0}' is not a recognized modifier or key")]
    UnknownToken(String),
    /// The same modifier token appeared more than once.
    #[error("the modifier '{0}' is repeated")]
    DuplicateModifier(String),
    /// The string started or ended with `+`, or contained `++` (an empty token between separators).
    #[error("a shortcut cannot start or end with '+', or contain '++'")]
    TrailingSeparator,
}

impl fmt::Display for Chord {
    /// The canonical round-tripping form: `mod+shift+alt+<key>`, each modifier token present only
    /// when held, in that fixed order (ADR 0030 §4).
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let mut tokens = Vec::new();
        if self.modifier.command {
            tokens.push("mod".to_owned());
        }
        if self.modifier.shift {
            tokens.push("shift".to_owned());
        }
        if self.modifier.alt {
            tokens.push("alt".to_owned());
        }
        tokens.push(key_token(self.key));
        write!(f, "{}", tokens.join("+"))
    }
}

impl FromStr for Chord {
    type Err = ChordParseError;

    /// Parses the canonical `mod+shift+alt+<key>` form (any subset of modifiers, in any order, but
    /// each at most once), the inverse of [`Chord`]'s `Display`.
    fn from_str(input: &str) -> Result<Self, Self::Err> {
        if input.is_empty() {
            return Err(ChordParseError::Empty);
        }
        let parts: Vec<&str> = input.split('+').collect();
        if parts.iter().any(|part| part.is_empty()) {
            return Err(ChordParseError::TrailingSeparator);
        }
        let (modifier_tokens, key_token_part) = parts.split_at(parts.len() - 1);
        let mut modifier = Modifier::NONE;
        for token in modifier_tokens {
            match *token {
                "mod" if !modifier.command => modifier.command = true,
                "mod" => return Err(ChordParseError::DuplicateModifier("mod".to_owned())),
                "shift" if !modifier.shift => modifier.shift = true,
                "shift" => return Err(ChordParseError::DuplicateModifier("shift".to_owned())),
                "alt" if !modifier.alt => modifier.alt = true,
                "alt" => return Err(ChordParseError::DuplicateModifier("alt".to_owned())),
                other => return Err(ChordParseError::UnknownToken(other.to_owned())),
            }
        }
        let key_token_str = key_token_part[0];
        let key =
            key_from_token(key_token_str).ok_or_else(|| ChordParseError::UnknownToken(key_token_str.to_owned()))?;
        Ok(Self { modifier, key })
    }
}

/// A shortcut's group in the `?` overlay.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ShortcutGroup {
    /// Global shortcuts, active anywhere. The only group a workspace may rebind (ADR 0030 §2).
    Global,
    /// `g`-prefix navigation (press `g`, then a category key). Not rebindable.
    Navigation,
    /// Within-screen shortcuts (list / tabs / facts). Not rebindable.
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

    /// The stable, kebab-case id this action is addressed by in client-scope `[shortcuts]` config
    /// (ADR 0030 §3) — the inverse of [`Self::from_config_id`].
    #[must_use]
    pub const fn config_id(self) -> &'static str {
        match self {
            Self::CommandPalette => "command-palette",
            Self::NewRecord => "new-record",
            Self::Find => "find",
            Self::Undo => "undo",
            Self::Redo => "redo",
            Self::SwitchRecordTab => "switch-record-tab",
            Self::DockRecordTab => "dock-record-tab",
            Self::Help => "help",
            Self::Close => "close",
            Self::MoveUp => "move-up",
            Self::MoveDown => "move-down",
            Self::Open => "open",
            Self::PrevRecord => "prev-record",
            Self::NextRecord => "next-record",
            Self::PrevTab => "prev-tab",
            Self::NextTab => "next-tab",
            Self::FirstTab => "first-tab",
            Self::LastTab => "last-tab",
            Self::AddSource => "add-source",
            Self::Edit => "edit",
            Self::Quit => "quit",
            Self::CloseCurrentTab => "close-tab",
        }
    }

    /// Resolves a config id back to its action, the inverse of [`Self::config_id`]. `None` for an id
    /// that names no action (an [`crate::BindingError::UnknownAction`]).
    #[must_use]
    pub fn from_config_id(id: &str) -> Option<Self> {
        Self::all().into_iter().find(|action| action.config_id() == id)
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

/// The complete default global + within-screen shortcut map, in `?`-overlay order.
///
/// The `g`-prefix navigation rows are *not* here — use [`navigation_shortcuts`] for those, so the
/// rail and overlay share one source. This is the *default* map; [`resolved_shortcuts`] is what the
/// dispatcher and overlay actually read once a workspace may have overrides.
#[must_use]
pub fn shortcuts() -> Vec<Shortcut> {
    use Key::{
        ArrowDown, ArrowLeft, ArrowRight, ArrowUp, BracketLeft, BracketRight, Char, DigitRange, End, Enter, Escape,
        Home, Question,
    };
    use ShortcutAction::{
        AddSource, Close, CloseCurrentTab, CommandPalette, DockRecordTab, Edit, Find, FirstTab, Help, LastTab,
        MoveDown, MoveUp, NewRecord, NextRecord, NextTab, Open, PrevRecord, PrevTab, Quit, Redo, SwitchRecordTab, Undo,
    };
    use ShortcutGroup::{Global, WithinScreen};
    let no_mod = Modifier::NONE;
    let command = Modifier::COMMAND;
    let command_shift = Modifier::COMMAND_SHIFT;
    vec![
        shortcut(CommandPalette, command, Char('k'), Global, "sc-command-palette"),
        shortcut(NewRecord, command, Char('n'), Global, "sc-new-record"),
        shortcut(Find, command, Char('f'), Global, "sc-find"),
        shortcut(Undo, command, Char('z'), Global, "sc-undo"),
        shortcut(Redo, command_shift, Char('z'), Global, "sc-redo"),
        shortcut(SwitchRecordTab, command, DigitRange, Global, "sc-switch-tab"),
        shortcut(DockRecordTab, command_shift, DigitRange, Global, "sc-dock-tab"),
        shortcut(Help, no_mod, Question, Global, "sc-help"),
        shortcut(Close, no_mod, Escape, Global, "sc-close"),
        shortcut(Quit, command, Char('q'), Global, "sc-quit"),
        shortcut(CloseCurrentTab, command, Char('w'), Global, "sc-close-tab"),
        shortcut(MoveUp, no_mod, ArrowUp, WithinScreen, "sc-move-up"),
        shortcut(MoveDown, no_mod, ArrowDown, WithinScreen, "sc-move-down"),
        shortcut(Open, no_mod, Enter, WithinScreen, "sc-open"),
        shortcut(PrevRecord, no_mod, BracketLeft, WithinScreen, "sc-prev-record"),
        shortcut(NextRecord, no_mod, BracketRight, WithinScreen, "sc-next-record"),
        shortcut(PrevTab, no_mod, ArrowLeft, WithinScreen, "sc-prev-tab"),
        shortcut(NextTab, no_mod, ArrowRight, WithinScreen, "sc-next-tab"),
        shortcut(FirstTab, no_mod, Home, WithinScreen, "sc-first-tab"),
        shortcut(LastTab, no_mod, End, WithinScreen, "sc-last-tab"),
        shortcut(AddSource, no_mod, Char('s'), WithinScreen, "sc-add-source"),
        shortcut(Edit, no_mod, Char('e'), WithinScreen, "sc-edit"),
    ]
}

/// A rejected override from [`resolved_shortcuts`] (ADR 0030 §4): the named action keeps its default
/// chord rather than being silently dropped.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum BindingError {
    /// The config id names no [`ShortcutAction`].
    #[error("'{id}' is not a known shortcut")]
    UnknownAction {
        /// The unrecognized config id.
        id: String,
    },
    /// The chord string could not be parsed.
    #[error("'{chord}' for '{id}' could not be parsed: {source}")]
    UnparsableChord {
        /// The action's config id.
        id: String,
        /// The unparsable chord string.
        chord: String,
        /// Why it failed to parse.
        source: ChordParseError,
    },
    /// The action named is not in [`ShortcutGroup::Global`] (within-screen and `g`-prefix keys are
    /// fixed, ADR 0030 §2).
    #[error("'{id}' is not a rebindable (Global-group) shortcut")]
    NotRebindable {
        /// The non-rebindable action's config id.
        id: String,
    },
    /// The parsed chord collides with another binding already resolved.
    #[error("'{chord}' for '{id}' collides with another shortcut")]
    Conflict {
        /// The action's config id.
        id: String,
        /// The colliding chord string.
        chord: String,
    },
}

/// Merges `overrides` (config id → canonical chord string, e.g. from client-scope `[shortcuts]`
/// config) over the default [`shortcuts`] map, producing the resolved list every renderer — the
/// dispatcher and the `?` overlay — reads (ADR 0030 §1), plus any rejected overrides.
///
/// `overrides` is processed in key order (a `BTreeMap`'s iteration order, so deterministic); each
/// candidate is checked against the map as resolved so far, so a chord that would collide with an
/// already-accepted override, or with an untouched default, is rejected — its action keeps the
/// default. Unlisted actions keep their default chord.
#[must_use]
pub fn resolved_shortcuts(overrides: &BTreeMap<String, String>) -> (Vec<Shortcut>, Vec<BindingError>) {
    let mut resolved = shortcuts();
    let mut errors = Vec::new();
    for (id, chord_string) in overrides {
        let Some(index) = ShortcutAction::from_config_id(id)
            .and_then(|action| resolved.iter().position(|entry| entry.action == action))
        else {
            errors.push(BindingError::UnknownAction { id: id.clone() });
            continue;
        };
        if resolved[index].group != ShortcutGroup::Global {
            errors.push(BindingError::NotRebindable { id: id.clone() });
            continue;
        }
        let chord = match Chord::from_str(chord_string) {
            Ok(chord) => chord,
            Err(source) => {
                errors.push(BindingError::UnparsableChord {
                    id: id.clone(),
                    chord: chord_string.clone(),
                    source,
                });
                continue;
            }
        };
        let collides = resolved
            .iter()
            .enumerate()
            .any(|(other_index, entry)| other_index != index && entry.chord == chord);
        if collides {
            errors.push(BindingError::Conflict {
                id: id.clone(),
                chord: chord_string.clone(),
            });
            continue;
        }
        resolved[index].chord = chord;
    }
    (resolved, errors)
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
    use std::collections::{BTreeMap, BTreeSet};
    use std::str::FromStr;

    use super::{
        BindingError, Chord, ChordParseError, Modifier, ShortcutAction, ShortcutGroup, navigation_shortcuts,
        resolved_shortcuts, shortcuts,
    };
    use crate::navigation::Category;

    /// Asserts every chord in `map` is unique (the invariant both the default and resolved maps must
    /// hold).
    fn assert_no_duplicate_chords(map: &[super::Shortcut]) {
        let chords: Vec<Chord> = map.iter().map(|entry| entry.chord).collect();
        for (index, chord) in chords.iter().enumerate() {
            assert_eq!(
                chords.iter().filter(|other| *other == chord).count(),
                1,
                "duplicate chord at {index}"
            );
        }
    }

    #[test]
    fn no_duplicate_chords() {
        assert_no_duplicate_chords(&shortcuts());
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
        assert_eq!(quit.chord.modifier, Modifier::COMMAND);
        assert_eq!(quit.label_id, "sc-quit");
        assert_eq!(close_tab.group, ShortcutGroup::Global);
        assert_eq!(close_tab.chord.modifier, Modifier::COMMAND);
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
        assert_eq!(undo.chord.modifier, Modifier::COMMAND);
        assert_eq!(redo.chord.modifier, Modifier::COMMAND_SHIFT);
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

    #[test]
    fn every_default_chord_round_trips_through_its_canonical_string() {
        for entry in shortcuts() {
            let canonical = entry.chord.to_string();
            assert_eq!(
                Chord::from_str(&canonical),
                Ok(entry.chord),
                "{:?} did not round-trip through {canonical:?}",
                entry.chord
            );
        }
    }

    #[test]
    fn chord_parse_rejects_an_empty_string() {
        assert_eq!(Chord::from_str(""), Err(ChordParseError::Empty));
    }

    #[test]
    fn chord_parse_rejects_an_unknown_token() {
        assert_eq!(
            Chord::from_str("mod+xyz"),
            Err(ChordParseError::UnknownToken("xyz".to_owned()))
        );
        assert_eq!(
            Chord::from_str("ctrl+k"),
            Err(ChordParseError::UnknownToken("ctrl".to_owned())),
            "the modifier token is 'mod', not a platform-specific name"
        );
    }

    #[test]
    fn chord_parse_rejects_a_duplicate_modifier() {
        assert_eq!(
            Chord::from_str("mod+mod+k"),
            Err(ChordParseError::DuplicateModifier("mod".to_owned()))
        );
    }

    #[test]
    fn chord_parse_rejects_a_trailing_or_doubled_separator() {
        assert_eq!(Chord::from_str("mod+k+"), Err(ChordParseError::TrailingSeparator));
        assert_eq!(Chord::from_str("+mod+k"), Err(ChordParseError::TrailingSeparator));
        assert_eq!(Chord::from_str("mod++k"), Err(ChordParseError::TrailingSeparator));
    }

    #[test]
    fn config_ids_are_unique_and_total_over_all_actions() {
        let ids: BTreeSet<&str> = ShortcutAction::all().iter().map(|action| action.config_id()).collect();
        assert_eq!(ids.len(), ShortcutAction::all().len(), "every config id is unique");
        for action in ShortcutAction::all() {
            assert_eq!(ShortcutAction::from_config_id(action.config_id()), Some(action));
        }
        assert_eq!(ShortcutAction::from_config_id("not-a-real-action"), None);
    }

    #[test]
    fn resolved_shortcuts_with_no_overrides_matches_the_default_map() {
        let (resolved, errors) = resolved_shortcuts(&BTreeMap::new());
        assert_eq!(resolved, shortcuts());
        assert!(errors.is_empty());
    }

    #[test]
    fn resolved_shortcuts_rejects_an_unknown_action_id_and_keeps_defaults() {
        let overrides = BTreeMap::from([("not-a-real-action".to_owned(), "mod+j".to_owned())]);
        let (resolved, errors) = resolved_shortcuts(&overrides);
        assert_eq!(resolved, shortcuts(), "an unknown id changes nothing");
        assert_eq!(
            errors,
            vec![BindingError::UnknownAction {
                id: "not-a-real-action".to_owned()
            }]
        );
    }

    #[test]
    fn resolved_shortcuts_rejects_an_unparsable_chord_and_keeps_that_actions_default() {
        let overrides = BTreeMap::from([("quit".to_owned(), "not a chord".to_owned())]);
        let (resolved, errors) = resolved_shortcuts(&overrides);
        let quit = resolved
            .iter()
            .find(|entry| entry.action == ShortcutAction::Quit)
            .expect("quit present");
        assert_eq!(quit.chord, shortcuts()[9].chord, "quit keeps its default chord");
        assert_eq!(errors.len(), 1);
        assert!(matches!(&errors[0], BindingError::UnparsableChord { id, .. } if id == "quit"));
    }

    #[test]
    fn resolved_shortcuts_rejects_overriding_a_non_global_action() {
        let overrides = BTreeMap::from([("move-up".to_owned(), "mod+u".to_owned())]);
        let (resolved, errors) = resolved_shortcuts(&overrides);
        assert_eq!(resolved, shortcuts(), "a within-screen action cannot be rebound");
        assert_eq!(
            errors,
            vec![BindingError::NotRebindable {
                id: "move-up".to_owned()
            }]
        );
    }

    #[test]
    fn resolved_shortcuts_rejects_a_chord_colliding_with_another_resolved_binding() {
        // "close-tab"'s default is `mod+w`; overriding "quit" to the same chord must collide.
        let overrides = BTreeMap::from([("quit".to_owned(), "mod+w".to_owned())]);
        let (resolved, errors) = resolved_shortcuts(&overrides);
        assert_eq!(resolved, shortcuts(), "the colliding override changes nothing");
        assert_eq!(
            errors,
            vec![BindingError::Conflict {
                id: "quit".to_owned(),
                chord: "mod+w".to_owned()
            }]
        );
    }

    #[test]
    fn resolved_shortcuts_applies_a_valid_override_and_stays_duplicate_free() {
        let overrides = BTreeMap::from([("quit".to_owned(), "mod+shift+q".to_owned())]);
        let (resolved, errors) = resolved_shortcuts(&overrides);
        assert!(errors.is_empty());
        let quit = resolved
            .iter()
            .find(|entry| entry.action == ShortcutAction::Quit)
            .expect("quit present");
        assert_eq!(quit.chord, Chord::from_str("mod+shift+q").expect("parses"));
        assert_no_duplicate_chords(&resolved);
    }
}
