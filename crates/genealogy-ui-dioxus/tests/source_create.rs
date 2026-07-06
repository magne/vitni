//! SSR assertions for the Source create form (Phase 5 PR26): the read-first draft create pane in the
//! detail pane renders a "draft · not saved" header, the labelled bibliographic fields, the
//! provenance block, and a Save/Cancel actions row with Save disabled while the draft is empty
//! (`record-editing.html` §6). Rendered from the pure field fn + shared header/actions so no `AppCtx`
//! is needed.

use dioxus::prelude::*;
use genealogy_ui::{Localizer, ProvenanceDraft, SourceDraft};
use genealogy_ui_dioxus::screens::{RecordActions, create_record_header, provenance_block, source_create_fields};

/// Renders the source create pane exactly as `SourceCreateRecord` composes it, but from the pure
/// pieces so the test needs no app context. `dirty` toggles the empty vs filled draft.
fn create_view(dirty: bool) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let draft = use_signal(|| {
        if dirty {
            SourceDraft {
                title: "Trinity Church baptisms".to_owned(),
                ..SourceDraft::new()
            }
        } else {
            SourceDraft::new()
        }
    });
    let prov = use_signal(ProvenanceDraft::default);
    let can_save = draft().is_dirty();
    rsx! {
        {create_record_header(&loc.source_new_title(), &loc.record_draft_badge())}
        {source_create_fields(&loc, draft)}
        {provenance_block(&loc, prov)}
        RecordActions {
            save_label: loc.action_label("save"),
            cancel_label: loc.action_label("cancel"),
            can_save,
            onsave: move |()| {},
            oncancel: move |()| {},
        }
    }
}

fn empty_view() -> Element {
    create_view(false)
}

fn dirty_view() -> Element {
    create_view(true)
}

#[test]
fn create_pane_shows_the_draft_badge_and_labelled_fields() {
    let mut vdom = VirtualDom::new(empty_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "New source",        // the draft title header
        "draft · not saved", // the unsaved-draft badge
        "Title",             // labelled fields (source.html create specimen)
        "Author",
        "Publication",
        "Abbreviation",
        r#"id="source-title""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn save_is_disabled_while_the_draft_is_empty_and_enabled_once_dirty() {
    let mut empty = VirtualDom::new(empty_view);
    empty.rebuild_in_place();
    let empty_html = dioxus_ssr::render(&empty);
    assert!(
        empty_html.contains("disabled"),
        "Save is disabled until the draft is dirty:\n{empty_html}"
    );

    let mut dirty = VirtualDom::new(dirty_view);
    dirty.rebuild_in_place();
    let dirty_html = dioxus_ssr::render(&dirty);
    assert!(
        !dirty_html.contains("disabled"),
        "Save is enabled once a field is filled:\n{dirty_html}"
    );
}
