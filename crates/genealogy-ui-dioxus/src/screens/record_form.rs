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
use genealogy_ui::{Category, Localizer, ProvenanceDraft, RecordDraft};

use crate::components::{Button, ButtonVariant};
use crate::screens::provenance_block;
use crate::services::Services;
use crate::shell::nav_state::NavState;

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

/// The edit state for an existing record, seeded from its current values. Reseeds the draft when the
/// committed record changes underneath (e.g. after a save reload) — but only while not editing, so a
/// live edit is never clobbered (the tag editor's precedent).
pub fn use_record_edit<D: RecordDraft>(seed: &D) -> RecordEditState<D> {
    let editing = use_signal(|| false);
    let mut seed_sig = use_signal({
        let seed = seed.clone();
        move || seed
    });
    let mut draft = use_signal({
        let seed = seed.clone();
        move || seed
    });
    let prov = use_signal(ProvenanceDraft::default);
    let seed = seed.clone();
    use_effect(use_reactive!(|seed| {
        if !editing() {
            seed_sig.set(seed.clone());
            draft.set(seed);
        }
    }));
    RecordEditState {
        editing,
        seed: seed_sig,
        draft,
        prov,
    }
}

/// The edit state for a create pane: an empty draft, in edit mode from the start (`record-editing.html`
/// §6). Never reseeds — a create draft is thrown away on Cancel.
pub fn use_record_create<D: RecordDraft>() -> RecordEditState<D> {
    RecordEditState {
        editing: use_signal(|| true),
        seed: use_signal(D::default),
        draft: use_signal(D::default),
        prov: use_signal(ProvenanceDraft::default),
    }
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
/// so the shell's overlay-close does not also fire) and Ctrl/⌘+`s` saves when the draft can be saved.
/// Typing an unmodified `s`/`e` inside an input never reaches here — the inputs stop that propagation
/// via `keep_typing_local`.
pub fn record_keydown<D: RecordDraft>(
    event: &KeyboardEvent,
    mut state: RecordEditState<D>,
    on_save: Callback<(D, ProvenanceDraft)>,
) {
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
    } else if typed == Some("s") && chord && state.can_save() {
        event.prevent_default();
        on_save.call((state.draft.read().clone(), state.prov.read().clone()));
        state.editing.set(false);
    }
}

/// Finishes a whole-record save: on success marks the workspace changed and either re-keys the open
/// tab to the record's new `human_id` (a rename remounts the detail pane by the new id) or bumps
/// `reload` to refetch; either way shows the saved toast. On failure shows the error toast.
pub fn finish_record_save(
    effective: Result<String, String>,
    category: Category,
    current: &str,
    mut nav: NavState,
    mut reload: Signal<u32>,
    mut toast: Signal<Option<String>>,
    saved: &str,
) {
    match effective {
        Ok(effective) => {
            nav.mark_changed();
            if effective == current {
                reload += 1;
            } else {
                nav.rename_record(category, current, effective);
            }
            toast.set(Some(saved.to_owned()));
        }
        Err(message) => toast.set(Some(message)),
    }
}
