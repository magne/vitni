//! The shared whole-record edit state and header actions (`record-editing.html`), generic over any
//! [`RecordDraft`]. One mechanism drives create, view, and edit for every aggregate: a record is
//! read-first, one Edit turns it into a buffered draft, and Save is gated on the draft being both
//! dirty and valid.
//!
//! No generic `#[component]`s: [`RecordEditState`] is a plain `Copy` bundle of signals and the
//! renderers are pure `fn`s, so the markup builders render under SSR without an `AppCtx`. Field
//! lensing (which draft field an input binds to) stays in the per-screen `*_record_fields` closures.

use std::future::Future;

use dioxus::prelude::*;
use genealogy_ui::{Category, Localizer, ProvenanceDraft, RecordDraft, RecordRef};

use crate::components::{Button, ButtonVariant};
use crate::screens::provenance_block;
use crate::services::Services;
use crate::shell::nav_state::{EditKey, NavState, StashedEdit};

/// The buffered edit state of one record: whether it is being edited, the committed `seed`, the live
/// `draft`, and the provenance collected once per save (`record-editing.html` §5b).
#[derive(Clone)]
pub struct RecordEditState<D: RecordDraft> {
    /// Whether the record is in edit mode (inputs) rather than view mode (read boxes).
    pub editing: Signal<bool>,
    /// The committed values the draft reverts to and is diffed against for dirtiness.
    pub seed: Signal<D>,
    /// The live, buffered draft the form binds to.
    pub draft: Signal<D>,
    /// The provenance (why / citations / evidence / confidence) applied to every saved assertion.
    pub prov: Signal<ProvenanceDraft>,
}

// `Signal<T>` is `Copy` for any `T: 'static`, so the whole bundle is `Copy` regardless of `D`; a
// derived `Copy` would wrongly demand `D: Copy`, so the `Copy` impl is written by hand (`Clone` is
// derived, gated on the always-satisfied `D: RecordDraft: Clone`).
impl<D: RecordDraft> Copy for RecordEditState<D> {}

impl<D: RecordDraft> RecordEditState<D> {
    /// Whether the draft differs from its committed seed.
    #[must_use]
    pub fn is_dirty(&self) -> bool {
        self.draft.read().is_dirty_against(&self.seed.read())
    }

    /// Whether Save is enabled: the draft is both dirty and valid (`record-editing.html` §5).
    #[must_use]
    pub fn can_save(&self) -> bool {
        self.is_dirty() && self.draft.read().is_valid()
    }

    /// Enters edit mode.
    pub fn begin_edit(&mut self) {
        self.editing.set(true);
    }

    /// Cancels the edit session: restores the draft to the seed, drops the provenance, returns to
    /// view mode (`record-editing.html` §5).
    pub fn cancel(&mut self) {
        let seed = self.seed.read().clone();
        self.draft.set(seed);
        self.prov.set(ProvenanceDraft::default());
        self.editing.set(false);
    }
}

/// The edit state for an existing record `(category, human_id)`, seeded from its current values.
/// Reseeds the draft when the committed record changes underneath (e.g. after a save reload) — but
/// only while not editing, so a live edit is never clobbered (the tag editor's precedent).
///
/// The buffer's **storage is the shell**, not this screen ([`NavState::edit_drafts`]): the detail panes
/// are keyed per record, so activating another tab unmounts this one, and a pane-local buffer would go
/// with it (issue #239). The buffer is therefore hydrated from the shell on mount
/// ([`use_stashed_edit`]) and written through on every change ([`use_edit_write_through`]), which also
/// makes the shell's dirty set — the tabstrip marker, `⌘W`/`⌘Q` — exact for every open tab at once.
///
/// A restored buffer comes up in edit mode, which is also what keeps the reseed below from clobbering
/// it when the record's detail finishes loading underneath.
pub fn use_record_edit<D: RecordDraft>(category: Category, human_id: &str, seed: &D) -> RecordEditState<D> {
    // The detail panes are keyed on `human_id` (`screens/record_detail.rs`), so a different record
    // remounts this hook rather than re-running it with a new id — the key captured here stays right.
    let key = EditKey::saved(category, human_id);
    // A stored record is read-first, so a buffer with nothing parked for it starts in view mode (§1).
    let state = use_stashed_edit(&key, false, seed);
    let mut seed_sig = state.seed;
    let mut draft = state.draft;
    let editing = state.editing;
    let seed = seed.clone();
    use_effect(use_reactive!(|seed| {
        if !editing() {
            seed_sig.set(seed.clone());
            draft.set(seed);
        }
    }));
    use_edit_write_through(key, state);
    state
}

/// The edit state for a create pane: the category's create draft as parked in the shell if one is in
/// progress, otherwise an empty draft — in edit mode from the start either way
/// (`record-editing.html` §6). Never reseeds; the draft is discarded on Cancel
/// ([`NavState::cancel_draft`]) and spent by Save ([`NavState::commit_draft`]), both of which drop the
/// parked entry.
#[must_use]
pub fn use_record_create<D: RecordDraft>(category: Category) -> RecordEditState<D> {
    let key = EditKey::draft(category);
    // A create form has nothing to read, so a fresh buffer starts in edit mode.
    let state = use_stashed_edit(&key, true, &D::default());
    use_edit_write_through(key, state);
    state
}

/// The contents a record's edit buffer comes up with on mount (see [`use_stashed_edit`]).
#[derive(Clone)]
struct EditSeed<D> {
    /// Whether the buffer starts in edit mode.
    editing: bool,
    /// The committed values the draft is diffed against.
    seed: D,
    /// The live draft.
    draft: D,
    /// The provenance collected for the pending save.
    prov: ProvenanceDraft,
}

/// Builds a record's edit buffer on mount: the edit parked in the shell for `key`, if there is one (its
/// pane was unmounted mid-edit), otherwise a fresh buffer over `seed` in `editing_when_fresh` mode. A
/// *restored* buffer always starts in edit mode — it holds changes the user has not saved.
///
/// The lookup runs in a hook, not an effect, so the *first* render already shows the restored draft
/// rather than flashing the committed record.
fn use_stashed_edit<D: RecordDraft>(key: &EditKey, editing_when_fresh: bool, seed: &D) -> RecordEditState<D> {
    let nav = use_context::<NavState>();
    let key = key.clone();
    let fresh = seed.clone();
    let initial = use_hook(move || match nav.stashed_edit::<D>(&key) {
        Some((draft, seed, prov)) => EditSeed {
            editing: true,
            seed,
            draft,
            prov,
        },
        None => EditSeed {
            editing: editing_when_fresh,
            seed: fresh.clone(),
            draft: fresh,
            prov: ProvenanceDraft::default(),
        },
    });
    let EditSeed {
        editing,
        seed,
        draft,
        prov,
    } = initial;
    RecordEditState {
        editing: use_signal(move || editing),
        seed: use_signal(move || seed),
        draft: use_signal(move || draft),
        prov: use_signal(move || prov),
    }
}

/// Writes the edit buffer through to the shell under `key`, so leaving the tab parks the edit instead of
/// discarding it. While the draft is dirty its contents are kept parked; the moment it is clean again —
/// Cancel restoring the seed, or a save reseeding it — the entry is dropped. Save and Cancel therefore
/// need no wiring of their own, and the parked keyset stays an exact dirty set.
fn use_edit_write_through<D: RecordDraft>(key: EditKey, state: RecordEditState<D>) {
    let mut nav = use_context::<NavState>();
    use_effect(move || {
        if !state.is_dirty() {
            nav.drop_edit(&key);
            return;
        }
        let draft = state.draft.read().clone();
        let seed = state.seed.read().clone();
        let prov = state.prov.read().clone();
        nav.stash_edit(key.clone(), StashedEdit::new(draft, seed, prov));
    });
}

/// Runs this pane's own Save when the shell asks for it — the close/quit confirm's **Save** /
/// **Save all** (issue #240). `category` + `human_id` name the editor (`None` for a category's create
/// draft), `save` is the screen's existing save closure, exactly as its Save button calls it.
///
/// The shell cannot save generically: save is per-screen and differently shaped per aggregate, so
/// `NavState::save_then_close` / `save_all_then_quit` arm the request, activate the record's tab so
/// this pane is mounted, and this effect runs the screen's own commit. The outcome flows back through
/// [`finish_record_save`] / [`finish_draft_commit`], which is what lets the run continue to the next
/// record and finally close the tab or quit.
///
/// Fires **once per armed request** (the `ran` latch, peeked so writing it cannot re-trigger the
/// effect). A record queued for saving whose draft turns out not to be savable is reported as a
/// failure rather than left hanging — the run stops and every tab stays open.
///
/// Returns to view mode after `save` runs (`state.editing.set(false)`), the same as the pane's own
/// Save button (`record_head_actions`): without it a shell-driven save (the close/quit confirm's
/// **Save**, or `⌘S`) leaves the pane in edit mode with a stale `seed`, so the record still reads as
/// dirty, keeps its parked buffer, and keeps the tabstrip's unsaved marker lit.
pub fn use_save_on_request<D: RecordDraft>(
    category: Category,
    human_id: Option<&str>,
    mut state: RecordEditState<D>,
    save: Callback<()>,
) {
    let mut nav = use_context::<NavState>();
    let key = match human_id {
        Some(human_id) => EditKey::saved(category, human_id),
        None => EditKey::draft(category),
    };
    let mut ran = use_signal(|| false);
    use_effect(move || {
        let armed = nav.save_request.read().as_ref().map(|request| request.key.clone());
        if armed.as_ref() != Some(&key) {
            if *ran.peek() {
                ran.set(false);
            }
            return;
        }
        if *ran.peek() {
            return;
        }
        ran.set(true);
        if state.can_save() {
            save.call(());
            state.editing.set(false);
        } else {
            nav.note_save_finished(key.category, key.human_id.as_deref(), false);
        }
    });
}

/// The already-localized Edit / Save / Cancel labels the header actions need.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RecordActionLabels {
    /// The "Edit" action label (view mode).
    pub edit: String,
    /// The "Save" action label (edit mode).
    pub save: String,
    /// The "Cancel" action label (edit mode).
    pub cancel: String,
}

impl RecordActionLabels {
    /// Resolves the three labels via the localizer.
    #[must_use]
    pub fn resolve(loc: &Localizer) -> Self {
        Self {
            edit: loc.action_label("edit"),
            save: loc.action_label("save"),
            cancel: loc.action_label("cancel"),
        }
    }
}

/// The record header's right-aligned actions (`DetailContainer` `head-actions`): in view mode the
/// screen's `extra_actions` (e.g. Compare) then a primary Edit; in edit mode Cancel then a primary
/// Save, disabled until the draft is dirty and valid. On Save the state returns to view mode and the
/// screen's `on_save` commits.
pub fn record_head_actions<D: RecordDraft>(
    labels: &RecordActionLabels,
    mut state: RecordEditState<D>,
    extra_actions: Element,
    on_save: Callback<(D, ProvenanceDraft)>,
) -> Element {
    if !state.editing.read().to_owned() {
        return rsx! {
            {extra_actions}
            Button {
                label: labels.edit.clone(),
                variant: ButtonVariant::Primary,
                small: true,
                onclick: move |_| state.begin_edit(),
            }
        };
    }
    let can_save = state.can_save();
    rsx! {
        Button {
            label: labels.cancel.clone(),
            small: true,
            onclick: move |_| state.cancel(),
        }
        Button {
            label: labels.save.clone(),
            variant: ButtonVariant::Primary,
            small: true,
            disabled: !can_save,
            onclick: move |_| {
                if state.can_save() {
                    on_save.call((state.draft.read().clone(), state.prov.read().clone()));
                    state.editing.set(false);
                }
            },
        }
    }
}

/// The provenance block (`record-editing.html` §5b), rendered only while the draft is dirty (so a
/// pristine record shows nothing to fill in).
pub fn record_edit_provenance<D: RecordDraft>(loc: &Localizer, state: RecordEditState<D>) -> Element {
    if state.is_dirty() {
        provenance_block(loc, state.prov)
    } else {
        rsx! {}
    }
}

/// Applies a whole-record edit's per-field commands sequentially (a Model-C save: one audited
/// assertion per changed field — non-atomic by design, see the change-set memory), each through the
/// aggregate's `save` helper. Threads the effective `human_id` forward — every command returns the
/// record's current id, and a trailing `SetHumanId` returns the renamed one — so the caller reloads by
/// the right id. Stops at the first error, returning it (earlier commits stand; the caller reloads).
pub async fn apply_record_edits<E, Fut, F>(
    services: Services,
    edits: Vec<E>,
    prov: ProvenanceDraft,
    current: String,
    save: F,
) -> Result<String, String>
where
    F: Fn(Services, E, ProvenanceDraft) -> Fut,
    Fut: Future<Output = Result<String, String>>,
{
    let mut effective = current;
    for edit in edits {
        effective = save(services.clone(), edit, prov.clone()).await?;
    }
    Ok(effective)
}

/// The record-scope keyboard shortcuts (`record-editing.html` §9), attached to a converted detail
/// pane's wrapper. In view mode `e`/`F2` enters edit; in edit mode `Esc` cancels (stopping propagation
/// so the shell's overlay-close does not also fire). Save is the shell's `ShortcutAction::SaveRecord`
/// (`⌘S`, ADR 0030) — a rebindable `Global` chord dispatched from the shell root and routed here via
/// `use_save_on_request`, not a chord this pane's own keydown handles.
/// Typing an unmodified `s`/`e` inside an input never reaches here — the inputs stop that propagation
/// via `keep_typing_local`.
pub fn record_keydown<D: RecordDraft>(event: &KeyboardEvent, mut state: RecordEditState<D>) {
    let key = event.key();
    let modifiers = event.modifiers();
    let typed = if let Key::Character(character) = &key {
        Some(character.as_str())
    } else {
        None
    };
    let chord = modifiers.ctrl() || modifiers.meta();
    if !*state.editing.read() {
        if key == Key::F2 || (typed == Some("e") && !chord) {
            event.prevent_default();
            state.begin_edit();
        }
        return;
    }
    if key == Key::Escape {
        event.stop_propagation();
        state.cancel();
    }
}

/// Finishes a whole-record save: on success marks the workspace changed and either re-keys the open
/// tab to the record's new `human_id` (a rename remounts the detail pane by the new id) or bumps
/// `reload` to refetch; either way shows the shell's saved notice. On failure shows it as an error
/// notice, which stays until dismissed.
///
/// Reports the outcome to the shell last ([`NavState::note_save_finished`]), so a save the close/quit
/// confirm asked for resolves only once this pane has finished writing its own signals — the tab may
/// close on the way out. A save the user started themselves reports too; the shell ignores it unless
/// a run is waiting on that record.
pub fn finish_record_save(
    effective: Result<String, String>,
    category: Category,
    current: &str,
    mut nav: NavState,
    mut reload: Signal<u32>,
    saved: &str,
) {
    match effective {
        Ok(effective) => {
            nav.mark_changed();
            if effective == current {
                reload += 1;
            } else {
                nav.rename_record(category, current, effective.clone());
            }
            nav.notify(saved.to_owned());
            nav.note_save_finished(category, Some(&effective), true);
        }
        Err(message) => {
            nav.notify_error(message);
            nav.note_save_finished(category, Some(current), false);
        }
    }
}

/// Finishes a create form's commit: on success marks the workspace changed (so the Explorer list and
/// rail counts refetch, same as an edit save), the draft tab becomes the stored record in place
/// ([`NavState::commit_draft`]) labelled `label` — or the record's own id when that is empty or absent
/// — and shows `created` as the shell's confirmation notice (create had no completion feedback at all
/// before #208); on failure the error is shown as a sticky shell notice and the draft is left as it was.
///
/// Every `*CreateRecord` screen ends its save here, so the close/quit confirm's Save can drive a create
/// draft through the same path a saved record takes ([`use_save_on_request`]).
pub fn finish_draft_commit(
    committed: Result<String, String>,
    category: Category,
    label: Option<String>,
    created: String,
    mut nav: NavState,
) {
    match committed {
        Ok(human_id) => {
            nav.mark_changed();
            let label = label
                .filter(|label| !label.is_empty())
                .unwrap_or_else(|| human_id.clone());
            nav.commit_draft(RecordRef {
                category,
                human_id,
                label,
            });
            nav.notify(created);
            nav.note_save_finished(category, None, true);
        }
        Err(message) => {
            nav.notify_error(message);
            nav.note_save_finished(category, None, false);
        }
    }
}
