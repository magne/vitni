//! SSR assertions for the Citation create pane (Phase 5 PR27): the shared record frame in create mode
//! — a "draft · not saved" header with Cancel/Save in the sticky head — plus the required source
//! (existing id flagged while blank, §7), the page, and the record-level confidence + evidence axes.

use dioxus::prelude::*;
use genealogy_ui::{CitationDraft, CitationSourceKind, Localizer, ProvenanceDraft};
use genealogy_ui_dioxus::components::{Button, ButtonVariant};
use genealogy_ui_dioxus::screens::{
    RecordEditState, citation_create_fields, create_record_header, record_edit_provenance,
};

fn view(seed: CitationDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let record = RecordEditState::<CitationDraft> {
        editing: use_signal(|| true),
        seed: use_signal(CitationDraft::new),
        draft: use_signal(move || seed),
        prov: use_signal(ProvenanceDraft::default),
    };
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_label("save"), variant: ButtonVariant::Primary, small: true, disabled: !can_save, onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.citation_new_title(), &loc.record_draft_badge(), actions)}
        {citation_create_fields(&loc, record.draft)}
        {record_edit_provenance(&loc, record)}
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

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn a_blank_source_is_flagged_and_blocks_save() {
    let html = render(empty_view);
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
    let html = render(sourced_view);
    assert!(!html.contains("disabled"), "Save enabled with a source:\n{html}");
}

#[test]
fn a_new_source_reveals_the_title_field() {
    let html = render(new_source_view);
    assert!(
        html.contains(r#"id="citation-new-source-title""#),
        "the inline source title field shows:\n{html}"
    );
    assert!(
        !html.contains("disabled"),
        "a new source makes the draft valid + dirty:\n{html}"
    );
}
