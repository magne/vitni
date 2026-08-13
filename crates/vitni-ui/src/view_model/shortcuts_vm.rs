//! The customizable-shortcuts preferences card's view-model (ADR 0030 §1, §4).
//!
//! One row per rebindable ([`ShortcutGroup::Global`]) action, resolving a workspace's client-scope
//! `[shortcuts]` overrides against the default chord map ([`resolved_shortcuts`]) and pairing each row
//! with any rejected override for its own action, already localized. Rejections that cannot be
//! attached to a row (an unknown action id, or a non-rebindable action) are still surfaced, in
//! [`ShortcutsVm::general_errors`] — ADR 0030 §4 requires no override is ever silently dropped.

use vitni_app::ShortcutConfig;

use super::Localizer;
use crate::shortcuts::{BindingError, ShortcutAction, ShortcutGroup, resolved_shortcuts, shortcuts};

/// One rebindable action's row in the shortcuts preferences card.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutBindingVm {
    /// The action this row rebinds.
    pub action: ShortcutAction,
    /// The action's stable config id (the `[shortcuts]` key).
    pub config_id: String,
    /// The Fluent label id describing the action (e.g. `sc-quit`) — resolved by the *renderer's own*
    /// chrome catalogue (ADR 0008 §3), not this crate's [`Localizer`].
    pub label_id: &'static str,
    /// The default chord, as its canonical round-tripping string (`Chord`'s `Display`).
    pub default_chord: String,
    /// The chord actually in effect: the accepted override if any, else the default.
    pub current_chord: String,
    /// Whether [`Self::current_chord`] differs from [`Self::default_chord`] (an accepted override).
    pub is_overridden: bool,
    /// An already-localized message for a rejected override on this action, if any.
    pub error: Option<String>,
}

/// The customizable-shortcuts preferences card's view-model.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShortcutsVm {
    /// The rebindable rows, in default-map order.
    pub rows: Vec<ShortcutBindingVm>,
    /// Rejected overrides that named no rebindable row (an unknown action id, or a non-`Global`
    /// action) — surfaced generally so a rejection is never silently dropped.
    pub general_errors: Vec<String>,
}

/// Builds a [`ShortcutsVm`] from the client-scope `[shortcuts]` config: resolves `config`'s overrides
/// against the default map and localizes any rejection via `loc`.
#[must_use]
pub fn shortcuts_vm(config: &ShortcutConfig, loc: &Localizer) -> ShortcutsVm {
    let defaults = shortcuts();
    let (resolved, errors) = resolved_shortcuts(&config.bindings);
    let mut rows = Vec::new();
    for default in defaults.iter().filter(|entry| entry.group == ShortcutGroup::Global) {
        let config_id = default.action.config_id();
        let current = resolved
            .iter()
            .find(|entry| entry.action == default.action)
            .map_or(default.chord, |entry| entry.chord);
        let error = errors
            .iter()
            .find(|error| row_error_id(error) == Some(config_id))
            .map(|error| loc.shortcut_binding_error(error));
        rows.push(ShortcutBindingVm {
            action: default.action,
            config_id: config_id.to_owned(),
            label_id: default.label_id,
            default_chord: default.chord.to_string(),
            current_chord: current.to_string(),
            is_overridden: current != default.chord,
            error,
        });
    }
    let general_errors = errors
        .iter()
        .filter(|error| row_error_id(error).is_none())
        .map(|error| loc.shortcut_binding_error(error))
        .collect();
    ShortcutsVm { rows, general_errors }
}

/// The config id a row-attachable [`BindingError`] (`UnparsableChord`/`Conflict`) names, so its
/// message can be paired with that action's row. `None` for `UnknownAction`/`NotRebindable`, which
/// name no rebindable row (an unknown id, or a non-`Global` action never has a row) and so surface in
/// [`ShortcutsVm::general_errors`] instead.
fn row_error_id(error: &BindingError) -> Option<&str> {
    match error {
        BindingError::UnparsableChord { id, .. } | BindingError::Conflict { id, .. } => Some(id),
        BindingError::UnknownAction { .. } | BindingError::NotRebindable { .. } => None,
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use vitni_app::ShortcutConfig;

    use super::shortcuts_vm;
    use crate::i18n::Localizer;
    use crate::shortcuts::{ShortcutAction, ShortcutGroup, shortcuts};

    fn en() -> Localizer {
        Localizer::with_languages(None, &["en".parse().expect("tag")])
    }

    #[test]
    fn one_row_per_global_action_with_no_overrides() {
        let loc = en();
        let vm = shortcuts_vm(&ShortcutConfig::default(), &loc);
        let global_count = shortcuts()
            .into_iter()
            .filter(|entry| entry.group == ShortcutGroup::Global)
            .count();
        assert_eq!(vm.rows.len(), global_count);
        assert!(vm.rows.iter().all(|row| !row.is_overridden && row.error.is_none()));
        assert!(vm.general_errors.is_empty());
        let quit = vm
            .rows
            .iter()
            .find(|row| row.action == ShortcutAction::Quit)
            .expect("quit row present");
        assert_eq!(quit.config_id, "quit");
        assert_eq!(quit.label_id, "sc-quit");
        assert_eq!(quit.current_chord, quit.default_chord);
    }

    #[test]
    fn an_accepted_override_is_flagged_and_shows_the_new_chord() {
        let loc = en();
        let bindings = BTreeMap::from([("quit".to_owned(), "mod+shift+q".to_owned())]);
        let vm = shortcuts_vm(&ShortcutConfig { bindings }, &loc);
        let quit = vm
            .rows
            .iter()
            .find(|row| row.action == ShortcutAction::Quit)
            .expect("quit row present");
        assert!(quit.is_overridden);
        assert_eq!(quit.current_chord, "mod+shift+q");
        assert_ne!(quit.current_chord, quit.default_chord);
        assert!(quit.error.is_none());
    }

    #[test]
    fn an_unparsable_override_attaches_a_localized_error_to_its_row_and_keeps_the_default() {
        let loc = en();
        let bindings = BTreeMap::from([("quit".to_owned(), "not a chord".to_owned())]);
        let vm = shortcuts_vm(&ShortcutConfig { bindings }, &loc);
        let quit = vm
            .rows
            .iter()
            .find(|row| row.action == ShortcutAction::Quit)
            .expect("quit row present");
        assert!(!quit.is_overridden, "the default is kept when the override is rejected");
        assert_eq!(quit.current_chord, quit.default_chord);
        assert!(
            quit.error.as_deref().is_some_and(|message| message.contains("quit")),
            "the row carries a localized error message: {:?}",
            quit.error
        );
        assert!(vm.general_errors.is_empty());
    }

    #[test]
    fn a_conflicting_override_attaches_an_error_to_the_overridden_action_not_the_target() {
        let loc = en();
        // "close-tab"'s default is `mod+w`; overriding "quit" to the same chord must collide.
        let bindings = BTreeMap::from([("quit".to_owned(), "mod+w".to_owned())]);
        let vm = shortcuts_vm(&ShortcutConfig { bindings }, &loc);
        let quit = vm
            .rows
            .iter()
            .find(|row| row.action == ShortcutAction::Quit)
            .expect("quit row present");
        assert!(quit.error.is_some(), "the conflicting action carries the error");
        let close_tab = vm
            .rows
            .iter()
            .find(|row| row.action == ShortcutAction::CloseCurrentTab)
            .expect("close-tab row present");
        assert!(
            close_tab.error.is_none(),
            "the untouched target keeps its default with no error"
        );
    }

    #[test]
    fn an_unknown_action_id_surfaces_as_a_general_error_not_a_row_error() {
        let loc = en();
        let bindings = BTreeMap::from([("not-a-real-action".to_owned(), "mod+j".to_owned())]);
        let vm = shortcuts_vm(&ShortcutConfig { bindings }, &loc);
        assert!(vm.rows.iter().all(|row| row.error.is_none()));
        assert_eq!(vm.general_errors.len(), 1);
        assert!(vm.general_errors[0].contains("not-a-real-action"));
    }

    #[test]
    fn overriding_a_non_global_action_surfaces_as_a_general_error() {
        let loc = en();
        let bindings = BTreeMap::from([("move-up".to_owned(), "mod+u".to_owned())]);
        let vm = shortcuts_vm(&ShortcutConfig { bindings }, &loc);
        assert!(vm.rows.iter().all(|row| row.error.is_none()));
        assert_eq!(vm.general_errors.len(), 1);
        assert!(vm.general_errors[0].contains("move-up"));
    }
}
