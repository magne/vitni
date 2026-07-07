//! SSR assertions for the Note detail (Phase 5 PR27): the read-first Content record (id · type ·
//! content · language), its edit mode swapping in inputs plus the sticky-header Cancel/Save, the
//! language tab (primary language + translations), the references tab, and the tags panel.

use dioxus::prelude::*;
use genealogy_app::{NoteType, TagRef, UsingKind};
use genealogy_ui::{Localizer, NoteDetail, NoteDraft, ProvenanceDraft, TranslationVm, UsingRecordVm};
use genealogy_ui_dioxus::screens::{
    RecordActionLabels, RecordEditState, note_content_tab, note_language_tab, note_references_table, note_tags_panel,
    record_head_actions,
};

/// A representative note detail: a Research note in English with one Norwegian translation (by
/// magne), referenced by a person and an event, and one tag.
fn sample() -> NoteDetail {
    NoteDetail {
        human_id: "N0009".to_owned(),
        id: "0190-note-id".to_owned(),
        title: "Immigration year unresolved".to_owned(),
        note_type: Some(NoteType::Research),
        note_type_label: Some("Research".to_owned()),
        text: Some("Immigration year unresolved\n\nNeed to confirm the immigration year for John Smith.".to_owned()),
        language: Some("en".to_owned()),
        translations: vec![TranslationVm {
            language: Some("nb-NO".to_owned()),
            text: "Må bekrefte innvandringsåret for John Smith.".to_owned(),
            translator: Some("magne".to_owned()),
        }],
        references: vec![
            UsingRecordVm {
                kind: UsingKind::Person,
                human_id: "I0042".to_owned(),
                id: "0190-person-42".to_owned(),
                label: "John Smith".to_owned(),
                kind_label: "Person".to_owned(),
            },
            UsingRecordVm {
                kind: UsingKind::Event,
                human_id: "E0007".to_owned(),
                id: "0190-event-7".to_owned(),
                label: "Marriage".to_owned(),
                kind_label: "Event".to_owned(),
            },
        ],
        tags: vec![TagRef {
            id: "0190-secret-tag-id".to_owned(),
            name: "Needs sources".to_owned(),
            color: Some("#e0884a".to_owned()),
            priority: Some(2),
        }],
        restrictions: Vec::new(),
        history: Vec::new(),
    }
}

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn state(editing: bool) -> RecordEditState<NoteDraft> {
    let seed = NoteDraft::from_detail(&sample());
    RecordEditState {
        editing: use_signal(move || editing),
        seed: use_signal({
            let seed = seed.clone();
            move || seed
        }),
        draft: use_signal(move || seed),
        prov: use_signal(ProvenanceDraft::default),
    }
}

fn note_view() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(false);
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (NoteDraft, ProvenanceDraft)| {}))}
        {note_content_tab(&loc, &detail, record)}
        {note_language_tab(&loc, &detail)}
        {note_references_table(&loc, &detail.references)}
        {note_tags_panel(&loc, &detail, use_signal(|| None), use_callback(|_| {}), &detail.human_id)}
    }
}

fn note_edit() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(true);
    let detail = sample();
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (NoteDraft, ProvenanceDraft)| {}))}
        {note_content_tab(&loc, &detail, record)}
    }
}

#[test]
fn overview_is_read_first_with_an_edit_button_and_no_inputs() {
    let html = render(note_view);
    assert!(html.contains(">Edit<"), "view mode offers Edit:\n{html}");
    assert!(
        !html.contains("<input") && !html.contains("<textarea"),
        "no live inputs in view mode:\n{html}"
    );
    assert!(
        html.contains("Immigration year unresolved"),
        "the content is shown:\n{html}"
    );
}

#[test]
fn edit_mode_swaps_in_the_inputs_and_header_actions() {
    let html = render(note_edit);
    assert!(
        html.contains("<textarea"),
        "content becomes a textarea in edit mode:\n{html}"
    );
    assert!(
        html.contains(">Cancel<") && html.contains(">Save<"),
        "Cancel/Save in the header:\n{html}"
    );
    assert!(
        html.contains(r#"id="note-id""#),
        "the editable human id is present:\n{html}"
    );
    assert!(
        html.contains(r#"id="note-language""#),
        "the language input is present:\n{html}"
    );
}

#[test]
fn language_tab_shows_the_translation_and_translator() {
    let html = render(note_view);
    for needle in ["nb-NO", "Må bekrefte innvandringsåret for John Smith", "magne"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn references_list_every_record_that_uses_the_note() {
    let html = render(note_view);
    for needle in ["John Smith", "Person", "Marriage", "Event", "E0007"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let html = render(note_view);
    assert!(html.contains("Needs sources"), "tag name shown:\n{html}");
    assert!(html.contains("#e0884a"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
