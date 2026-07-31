//! Focus containment for the shell overlays.
//!
//! Two shapes, because the right containment depends on how much there is to move between.
//!
//! The command palette and the help sheet each have a single primary focusable control, so focus
//! enters declaratively via `autofocus` on that control and is contained by [`trap_tab`]:
//! `Tab`/`Shift+Tab` are swallowed outright, which is correct when there is nowhere else to go.
//!
//! A dialog with several controls — the close/quit confirm's Cancel / Discard / Save — cannot swallow
//! `Tab` without stranding a keyboard user on whichever button happens to hold focus, so it is
//! contained by the *cycling* trap instead: [`focus_guard`] brackets the dialog's content with a pair
//! of offscreen tab stops, and tabbing onto one wraps focus to the opposite end of the dialog. Moving
//! *between* the controls stays the browser's own `Tab` handling — the guards are the only place the
//! trap intervenes — so the trap needs no list of the dialog's controls and holds for whatever the
//! dialog contains. [`DialogFocus`] moves focus into the dialog when it opens and restores the control
//! that had it once the dialog closes (`docs/mockups/shortcuts.html`).
//!
//! `Esc` is handled by [`dismiss_on_escape`], attached to each overlay's own root. It cannot be left
//! to the shell's central keyboard dispatcher: that listener sits on `.app`, and every overlay is
//! rendered as a *sibling* of `.app` (so inerting `.app` cannot inert the overlay), which means a
//! keydown inside an overlay never reaches it.

use dioxus::prelude::*;

/// Swallows `Tab`/`Shift+Tab` so focus stays within a single-focusable overlay.
///
/// Attach to the overlay's dialog root `onkeydown`. Other keys are left to bubble — `Esc` to the
/// overlay root's own [`dismiss_on_escape`].
pub fn trap_tab(event: &KeyboardEvent) {
    if event.key() == Key::Tab {
        event.prevent_default();
    }
}

/// Runs `dismiss` when the key is `Esc`, so an overlay closes on it.
///
/// Attach to the overlay's **outermost** root (`div.overlay`), not the dialog: the shell's dispatcher
/// listens on `.app`, and overlays render as siblings of `.app`, so nothing else sees this keydown.
pub fn dismiss_on_escape(event: &KeyboardEvent, dismiss: impl FnOnce()) {
    if event.key() == Key::Escape {
        event.prevent_default();
        dismiss();
    }
}

/// One of the pair of offscreen tab stops bracketing a trapped dialog's content.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusGuard {
    /// The tab stop before the dialog's content.
    Leading,
    /// The tab stop after the dialog's content.
    Trailing,
}

/// An end of a trapped dialog's focusable controls.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DialogEnd {
    /// The dialog's first focusable control.
    First,
    /// Its last.
    Last,
}

/// The control focus wraps to when `guard` receives it — the whole of the trap's decision, since the
/// browser moves focus between the dialog's controls itself. `Tab` off the last control reaches the
/// trailing guard and wraps to the first; `Shift+Tab` off the first control reaches the leading guard
/// and wraps to the last.
#[must_use]
fn wrap_end(guard: FocusGuard) -> DialogEnd {
    match guard {
        FocusGuard::Leading => DialogEnd::Last,
        FocusGuard::Trailing => DialogEnd::First,
    }
}

/// Binds `dialog` to the open trapped dialog's root and `controls` to its focusable controls, in tab
/// order. The guards are excluded: focusing one would bounce focus straight back to the other.
const DIALOG_CONTROLS: &str = r#"
const dialog = document.querySelector('[data-focus-trap]');
const controls = dialog === null ? [] : Array.from(dialog.querySelectorAll(
  'a[href], button:not([disabled]), input:not([disabled]), select:not([disabled]),'
  + ' textarea:not([disabled]), [tabindex]:not([tabindex="-1"])'
)).filter((node) => !node.hasAttribute('data-focus-guard'));
"#;

/// The script that moves focus to one `end` of the open trapped dialog's controls, falling back to the
/// dialog itself when it has none (so focus still cannot land outside).
fn focus_end_script(end: DialogEnd) -> String {
    let index = match end {
        DialogEnd::First => "0",
        DialogEnd::Last => "controls.length - 1",
    };
    format!("{DIALOG_CONTROLS}\nif (dialog !== null) (controls[{index}] ?? dialog).focus();")
}

/// The script that moves focus into the open trapped dialog and restores the control that had it once
/// the dialog is gone. The restore is driven by the dialog's own removal (a `MutationObserver`) rather
/// than a Rust-side unmount hook, so it runs *after* the shell has torn the dialog down and lifted
/// `inert` from the background — an inert element cannot take focus.
fn enter_and_restore_script() -> String {
    format!(
        "{DIALOG_CONTROLS}
if (dialog !== null) {{
  const restore = document.activeElement;
  (controls[0] ?? dialog).focus();
  const observer = new MutationObserver(() => {{
    if (dialog.isConnected) return;
    observer.disconnect();
    requestAnimationFrame(() => {{
      if (restore instanceof HTMLElement && restore.isConnected && !dialog.contains(restore)) {{
        restore.focus();
      }}
    }});
  }});
  observer.observe(document.body, {{ childList: true, subtree: true }});
}}"
    )
}

/// Runs one of this module's scripts against the mounted dialog. A no-op under SSR, which has no
/// document to evaluate against.
fn run(script: String) {
    spawn(async move {
        let _ = document::eval(&script).await;
    });
}

/// One of a trapped dialog's focus guards: an offscreen tab stop that sends focus to the opposite end
/// of the dialog the moment it arrives, so `Tab`/`Shift+Tab` cycle within the dialog instead of
/// reaching the inert background. Hidden from assistive tech — focus never rests here.
pub fn focus_guard(guard: FocusGuard) -> Element {
    rsx! {
        span {
            class: "focus-guard",
            "data-focus-guard": "true",
            tabindex: "0",
            aria_hidden: "true",
            onfocusin: move |_| run(focus_end_script(wrap_end(guard))),
        }
    }
}

/// Arms the enclosing dialog's focus entry and restore-on-close. Markup-free, and mounted inside the
/// dialog, so it arms exactly once per open and needs no `open` flag of its own; the script runs from
/// an effect because it reads the mounted DOM.
#[component]
pub fn DialogFocus() -> Element {
    use_effect(|| run(enter_and_restore_script()));
    rsx! {}
}

/// `true` when a character key should stay in the focused text input rather than reach the shell's
/// global shortcut dispatcher.
///
/// Unmodified characters stay local (so `g`/`?`/… type instead of triggering shortcuts). A
/// primary-modifier chord (`⌘…`/`Ctrl…`) bubbles to the shell — *except* native text undo/redo
/// (`⌘Z`/`⌘⇧Z`), which must stay in the input so the webview edits the text rather than the shell
/// arming a record undo. `Esc`, `Tab`, and the arrow keys are not characters, so they bubble.
fn is_local_typing(key: &Key, modifiers: Modifiers) -> bool {
    let Key::Character(character) = key else {
        return false;
    };
    if !(modifiers.meta() || modifiers.ctrl()) {
        return true;
    }
    character.eq_ignore_ascii_case("z")
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
    use super::{DialogEnd, FocusGuard, enter_and_restore_script, focus_end_script, is_local_typing, wrap_end};
    use dioxus::prelude::{Key, Modifiers};

    #[test]
    fn each_guard_wraps_focus_to_the_opposite_end() {
        // Tab off the last control reaches the trailing guard, so it must go back to the first.
        assert_eq!(wrap_end(FocusGuard::Trailing), DialogEnd::First);
        // Shift+Tab off the first control reaches the leading guard, so it must go on to the last.
        assert_eq!(wrap_end(FocusGuard::Leading), DialogEnd::Last);
    }

    #[test]
    fn the_wrap_scripts_pick_opposite_ends_of_the_control_list() {
        let first = focus_end_script(DialogEnd::First);
        let last = focus_end_script(DialogEnd::Last);
        assert!(first.contains("(controls[0] ?? dialog).focus()"), "{first}");
        assert!(
            last.contains("(controls[controls.length - 1] ?? dialog).focus()"),
            "{last}"
        );
    }

    #[test]
    fn the_scripts_scope_themselves_to_the_dialog_and_skip_its_guards() {
        for script in [
            focus_end_script(DialogEnd::First),
            focus_end_script(DialogEnd::Last),
            enter_and_restore_script(),
        ] {
            assert!(
                script.contains("[data-focus-trap]"),
                "focus stays inside the dialog, not the page:\n{script}"
            );
            assert!(
                script.contains("!node.hasAttribute('data-focus-guard')"),
                "a guard is never a wrap target, or the two bounce focus between them:\n{script}"
            );
            assert!(
                script.contains(r#"[tabindex]:not([tabindex="-1"])"#),
                "the dialog root itself is not a control:\n{script}"
            );
            assert!(
                script.contains("button:not([disabled])"),
                "a disabled Save is not a tab stop:\n{script}"
            );
        }
    }

    #[test]
    fn the_entry_script_restores_the_previously_focused_control_after_the_dialog_goes() {
        let script = enter_and_restore_script();
        assert!(
            script.contains("const restore = document.activeElement"),
            "the control focused before the dialog opened is remembered:\n{script}"
        );
        assert!(
            script.contains("if (dialog.isConnected) return"),
            "the restore waits for the dialog to be removed:\n{script}"
        );
        assert!(
            script.contains("requestAnimationFrame"),
            "and for the background's inert to be lifted with it:\n{script}"
        );
    }

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
    fn primary_modified_undo_redo_stays_local() {
        // ⌘Z / Ctrl+Z and ⌘⇧Z (uppercase) keep native text undo/redo inside the input.
        for key in [Key::Character("z".to_string()), Key::Character("Z".to_string())] {
            assert!(is_local_typing(&key, Modifiers::META));
            assert!(is_local_typing(&key, Modifiers::CONTROL));
            assert!(is_local_typing(&key, Modifiers::META | Modifiers::SHIFT));
        }
    }

    #[test]
    fn non_character_keys_bubble() {
        for key in [Key::Escape, Key::Tab, Key::ArrowDown] {
            assert!(!is_local_typing(&key, Modifiers::empty()));
        }
    }
}
