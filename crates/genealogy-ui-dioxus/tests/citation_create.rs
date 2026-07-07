//! SSR assertions for the Citation create form (Phase 5 PR26): the draft header, the required source
//! (existing id flagged while blank, §7), the page, and the record-level confidence + three evidence
//! axis selects. Save gated on a resolvable source.

use dioxus::prelude::*;
use genealogy_ui::{CitationDraft, CitationSourceKind, Localizer, ProvenanceDraft};
use genealogy_ui_dioxus::screens::{RecordActions, citation_create_fields, create_record_header, provenance_block};

fn view(seed: CitationDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let draft = use_signal(move || seed);
    let prov = use_signal(ProvenanceDraft::default);
    let can_save = draft().is_dirty() && draft().is_valid();
    rsx! {
        {create_record_header(&loc.citation_new_title(), &loc.record_draft_badge(), rsx! {})}
        {citation_create_fields(&loc, draft)}
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
    view(CitationDraft::new())
}

fn sourced_view() -> Element {
    view(CitationDraft {
        existing_source: "S0001".to_owned(),
        ..CitationDraft::new()
    })
}

fn new_source_view() -> Element {
    view(CitationDraft {
        source_kind: CitationSourceKind::New,
        ..CitationDraft::new()
    })
}

#[test]
fn a_blank_source_is_flagged_and_blocks_save() {
    let mut vdom = VirtualDom::new(empty_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    for needle in [
        "New citation",
        "draft · not saved",
        r#"id="citation-source-kind""#,
        r#"id="citation-page""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert!(
        html.contains(r#"aria-invalid="true""#),
        "the blank existing source is flagged:\n{html}"
    );
    assert!(html.contains("A source is required"), "the field error shows:\n{html}");
    assert!(html.contains("disabled"), "Save blocked without a source:\n{html}");
}

#[test]
fn an_existing_source_enables_save() {
    let mut vdom = VirtualDom::new(sourced_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(!html.contains("disabled"), "Save enabled with a source:\n{html}");
}

#[test]
fn a_new_source_reveals_the_title_field() {
    let mut vdom = VirtualDom::new(new_source_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(
        html.contains(r#"id="citation-new-source-title""#),
        "the inline source title field shows:\n{html}"
    );
    assert!(
        !html.contains("disabled"),
        "a new source makes the draft valid + dirty:\n{html}"
    );
}
