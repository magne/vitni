//! Focus containment for the shell overlays.
//!
//! Two shapes, because the right containment depends on how much there is to move between.
//!
//! The command palette and the help sheet each have a single primary focusable control, so focus
//! enters declaratively via `autofocus` on that control and is contained by [`trap_tab`]:
//! `Tab`/`Shift+Tab` are swallowed outright, which is correct when there is nowhere else to go.
//!
//! A dialog with several controls — the close/quit confirm's Cancel / Discard / Save, or any record
//! side panel's form fields — cannot swallow
//! `Tab` without stranding a keyboard user on whichever button happens to hold focus, so it is
//! contained by the *cycling* trap instead: [`focus_guard`] brackets the dialog's content with a pair
//! of offscreen tab stops, and tabbing onto one wraps focus to the opposite end of the dialog. Moving
//! *between* the controls stays the browser's own `Tab` handling — the guards are the only place the
//! trap intervenes — so the trap needs no list of the dialog's controls and holds for whatever the
//! dialog contains. [`DialogFocus`] moves focus into the dialog when it opens and restores the control
//! that had it once the dialog closes (`docs/mockups/shortcuts.html`).
//!
//! `Esc` is handled by [`dismiss_on_escape`], attached to each dialog layer's own root. For the
//! overlays it cannot be left to the shell's central keyboard dispatcher: that listener sits on
//! `.app`, and every overlay is rendered as a *sibling* of `.app` (so inerting `.app` cannot inert the
//! overlay), which means a keydown inside an overlay never reaches it. A side panel is the mirror
//! case — it renders *inside* `.app`, so the dispatcher would see the keydown as well, and
//! [`dismiss_on_escape`] stops it.

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

/// Runs `dismiss` when the key is `Esc`, so a dialog closes on it.
///
/// Attach to the dialog layer's **outermost** root (`div.overlay`, or `div.sidepanel` for a panel).
/// An overlay needs it because the shell's dispatcher listens on `.app` and overlays render as
/// siblings of `.app`, so nothing else sees the keydown; a side panel renders *inside* `.app`, so the
/// keydown would otherwise reach the dispatcher as well — hence the `stop_propagation`, which keeps
/// one `Esc` from both closing the panel and dismissing whatever the shell considers topmost.
pub fn dismiss_on_escape(event: &KeyboardEvent, dismiss: impl FnOnce()) {
    if event.key() == Key::Escape {
        event.prevent_default();
        event.stop_propagation();
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
///
/// The *last* trapped element in the document wins, because that is the topmost layer: a side panel
/// renders inside `.app` while the overlays render as siblings after it, so a close/quit confirm
/// raised over an open side panel must trap in the confirm, not in the panel behind it.
const DIALOG_CONTROLS: &str = r#"
const traps = document.querySelectorAll('[data-focus-trap]');
const dialog = traps.length === 0 ? null : traps[traps.length - 1];
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

/// The script that records every element as it takes focus, so a dialog can still name the control
/// that opened it after the background went `inert` — becoming inert *blurs* whatever it contains
/// (`WebKit` runs the focus fixup rule), so by the time a side panel's own effect looks,
/// `document.activeElement` may already be the body (#312). Installed once, at shell mount, because
/// the control it has to remember was focused before the panel existed.
///
/// Focus guards are skipped: focus only ever passes through one on its way somewhere else, so one is
/// never what a dialog should restore to.
const TRACK_FOCUS: &str = "
if (!window.__vitniFocusTracked) {
  window.__vitniFocusTracked = true;
  document.addEventListener('focusin', (event) => {
    const target = event.target;
    if (target instanceof HTMLElement && !target.hasAttribute('data-focus-guard')) {
      window.__vitniLastFocused = target;
    }
  }, true);
}
";

/// How many further animation frames the restore will wait for the background to stop being `inert`.
/// A dialog's removal and the lifting of `inert` behind it are two different render passes, so they can
/// reach the webview as two batches — `focus()` on a still-inert control is silently ignored, and
/// nothing would try again (#312).
const RESTORE_FRAMES: u8 = 8;

/// The script that moves focus into the open trapped dialog and restores the control that had it once
/// the dialog is gone. The restore is driven by the dialog's own removal (a `MutationObserver`) rather
/// than a Rust-side unmount hook, so it runs *after* the shell has torn the dialog down; it then
/// retries for [`RESTORE_FRAMES`] frames, until the control is no longer `inert` and takes the focus.
///
/// The control to restore is the live `document.activeElement` whenever the browser still has one, and
/// [`TRACK_FOCUS`]'s record otherwise: a side panel inerts the pane it covers, which blurs the very
/// control that opened it, leaving `activeElement` as the body (#312).
fn enter_and_restore_script() -> String {
    format!(
        "{DIALOG_CONTROLS}
if (dialog !== null) {{
  const focused = document.activeElement;
  const restore = focused instanceof HTMLElement && focused !== document.body
    ? focused
    : window.__vitniLastFocused;
  (controls[0] ?? dialog).focus();
  const restoreFocus = (attempts) => {{
    if (!(restore instanceof HTMLElement) || !restore.isConnected || dialog.contains(restore)) return;
    restore.focus();
    if (document.activeElement !== restore && attempts > 0) {{
      requestAnimationFrame(() => restoreFocus(attempts - 1));
    }}
  }};
  const observer = new MutationObserver(() => {{
    if (dialog.isConnected) return;
    observer.disconnect();
    requestAnimationFrame(() => restoreFocus({RESTORE_FRAMES}));
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

/// Installs [`TRACK_FOCUS`] once for the whole application. Markup-free; mounted by the
/// [`Shell`](super::root::Shell) beside its other managers, because a dialog's restore target is
/// focused long before that dialog mounts.
#[component]
pub fn FocusHistory() -> Element {
    use_effect(|| run(TRACK_FOCUS.to_owned()));
    rsx! {}
}

/// Moves focus back to the open trapped dialog's first control — for a control that mounts *inside*
/// an already-open dialog and replaces whatever held focus there (an attach picker's "+ New …" card
/// replacing its search input, issue #314). [`DialogFocus`] only runs once, when the dialog itself
/// opens, so it never sees a later swap within the same open dialog; and an element that unmounts while
/// focused drops focus to `<body>` (outside `[data-focus-trap]`), which silently breaks both
/// `Esc`-to-dismiss and `Tab` cycling — the dialog looks unchanged, but the keyboard no longer reaches
/// it. Call from the replacement control's own `onmounted`.
pub fn refocus_dialog_start() {
    run(focus_end_script(DialogEnd::First));
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
    use super::{
        DialogEnd, FocusGuard, TRACK_FOCUS, enter_and_restore_script, focus_end_script, is_local_typing, wrap_end,
    };
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
            script.contains("const focused = document.activeElement"),
            "the control focused before the dialog opened is remembered:\n{script}"
        );
        assert!(
            script.contains("window.__vitniLastFocused"),
            "and the focus history covers the case where inerting the background already blurred it \
             (#312):\n{script}"
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
    fn the_focus_history_records_every_control_but_the_guards() {
        assert!(
            TRACK_FOCUS.contains("window.__vitniFocusTracked"),
            "the listener installs at most once:\n{TRACK_FOCUS}"
        );
        assert!(
            TRACK_FOCUS.contains("!target.hasAttribute('data-focus-guard')"),
            "a guard is never a restore target — focus only passes through one:\n{TRACK_FOCUS}"
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
