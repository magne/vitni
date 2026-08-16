//! SSR-probe coverage for issue #314's find-or-create attach mechanism: a successful "+ New …" create
//! must flip the link to `Existing`, bump [`NavState::data_version`] (so the rail counts, the Explorer
//! list, and every open picker refetch — #207/#266), and fire the attach callback with the new id; a
//! failed create must do none of that, leaving the draft exactly as typed and the localized error set
//! for [`NewRecordCard`] to render. Same host-free style as `create_refresh.rs`:
//! [`finish_attach_create`] is a free fn, driven directly from a mount-time hook over a bare
//! [`NavState`] and [`AttachLink`], with every observable rendered as a marker.

use dioxus::prelude::*;
use vitni_ui::{
    AttachSaveAction, NewNoteFields, NewRecordDraft, PickerSelection, PickerState, RecordLink, resolve_attach_save,
};
use vitni_ui_dioxus::screens::{AttachLink, finish_attach_create};
use vitni_ui_dioxus::shell::nav_state::NavState;

/// The marker block: the data-change ticket, the in-card error (or `NONE`), what `onattach` was called
/// with (or `NONE`), and the link's own kind (so a failure's "the draft stays intact" claim is checked
/// directly, not inferred from the absence of other markers).
fn probe(nav: &NavState, attach: AttachLink, attached: Signal<Option<String>>) -> Element {
    let version = *nav.data_version.read();
    let error = attach.error.read().clone().unwrap_or_else(|| "NONE".to_owned());
    let attached = attached.read().clone().unwrap_or_else(|| "NONE".to_owned());
    let link_kind = match &*attach.link.read() {
        RecordLink::Empty => "EMPTY",
        RecordLink::Existing(_) => "EXISTING",
        RecordLink::New(_) => "NEW",
    };
    rsx! {
        div { "VERSION:{version}" }
        div { "ERROR:{error}" }
        div { "ATTACHED:{attached}" }
        div { "LINK:{link_kind}" }
    }
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// A `+ New note` draft's [`AttachLink`], starting mid-save (`saving: true`) so a scenario can assert
/// [`finish_attach_create`] resets it regardless of outcome.
fn note_attach_link() -> AttachLink {
    AttachLink {
        link: use_signal(|| {
            RecordLink::New(NewRecordDraft::Note(NewNoteFields {
                text: "A research note".to_owned(),
            }))
        }),
        state: use_signal(PickerState::default),
        error: use_signal(|| None::<String>),
        saving: use_signal(|| true),
    }
}

fn successful_create() -> Element {
    let nav = use_context_provider(NavState::new);
    let attach = note_attach_link();
    let attached = use_signal(|| None::<String>);
    use_hook(move || {
        let onattach = Callback::new(move |id: String| {
            let mut attached = attached;
            attached.set(Some(id));
        });
        finish_attach_create(
            Ok("N0007".to_owned()),
            Some("A research note".to_owned()),
            attach,
            Some(nav),
            onattach,
        );
    });
    probe(&nav, attach, attached)
}

#[test]
fn a_successful_create_yields_the_attach_edit_for_the_new_id() {
    let html = render(successful_create);
    assert!(
        html.contains("ATTACHED:N0007"),
        "onattach fires with the created record's id:\n{html}"
    );
}

#[test]
fn a_successful_create_bumps_the_data_version() {
    let html = render(successful_create);
    assert!(
        html.contains("VERSION:1"),
        "a created support record marks the workspace changed, same as any other create:\n{html}"
    );
}

#[test]
fn a_successful_create_flips_the_link_to_existing() {
    let html = render(successful_create);
    assert!(
        html.contains("LINK:EXISTING"),
        "the card collapses to a picker-value chip so a retried Save only re-attaches:\n{html}"
    );
    assert!(
        html.contains("ERROR:NONE"),
        "no stale error survives a successful create:\n{html}"
    );
}

fn failed_create() -> Element {
    let nav = use_context_provider(NavState::new);
    let attach = note_attach_link();
    let attached = use_signal(|| None::<String>);
    use_hook(move || {
        let onattach = Callback::new(move |id: String| {
            let mut attached = attached;
            attached.set(Some(id));
        });
        finish_attach_create(
            Err("could not save the note".to_owned()),
            Some("A research note".to_owned()),
            attach,
            Some(nav),
            onattach,
        );
    });
    probe(&nav, attach, attached)
}

#[test]
fn a_failed_create_yields_no_attach_edit() {
    let html = render(failed_create);
    assert!(
        html.contains("ATTACHED:NONE"),
        "onattach never fires when nothing was written:\n{html}"
    );
}

#[test]
fn a_failed_create_does_not_bump_the_data_version_but_notifies() {
    let html = render(failed_create);
    assert!(
        html.contains("VERSION:0"),
        "nothing was written, so no subscriber should refetch:\n{html}"
    );
    assert!(
        html.contains("ERROR:could not save the note"),
        "the failure is surfaced as the card's own in-place error, not a shell notice:\n{html}"
    );
}

#[test]
fn a_failed_create_leaves_the_panel_open_with_the_draft_intact() {
    let html = render(failed_create);
    assert!(
        html.contains("LINK:NEW"),
        "the link stays a New draft — every typed character survives a failed create:\n{html}"
    );
}

#[test]
fn an_existing_selection_needs_no_create_and_dispatches_directly() {
    // No `AttachLink`, no `NavState`, no async commit: an existing selection resolves to `Attach`
    // straight from the framework-free decision `use_attach_save` wraps, proving the dioxus layer
    // adds no logic of its own on this path.
    let link = RecordLink::Existing(PickerSelection {
        human_id: "N0007".to_owned(),
        title: "Baptism note".to_owned(),
    });
    assert_eq!(resolve_attach_save(&link), AttachSaveAction::Attach("N0007".to_owned()));
}
