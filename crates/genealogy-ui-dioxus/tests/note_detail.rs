//! SSR assertions for the Note detail (Phase 5 PR10): render the content tab (type + rich text), the
//! language tab (primary language + the translations table with its translator), the references tab
//! (the reverse-index records), and the tags panel. Asserts the content, the translation + translator,
//! the referencing records, and that a tag shows its name/colour but never its id.

use dioxus::prelude::*;
use genealogy_app::{NoteType, TagRef, UsingKind};
use genealogy_ui::{Localizer, NoteDetail, TranslationVm, UsingRecordVm};
use genealogy_ui_dioxus::screens::{
    NoteEditForm, note_content_tab, note_language_tab, note_references_table, note_tags_panel,
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

/// Renders the content, language, references, and tags tabs together.
fn note_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let editing = use_signal(|| None::<NoteEditForm>);
    let on_submit = use_callback(|_edit: genealogy_ui::NoteEdit| {});
    let detail = sample();
    rsx! {
        {note_content_tab(&loc, &detail, editing)}
        {note_language_tab(&loc, &detail)}
        {note_references_table(&loc, &detail.references)}
        {note_tags_panel(&loc, &detail, editing, on_submit, &detail.human_id)}
    }
}

#[test]
fn content_shows_the_type_and_rich_text() {
    let mut vdom = VirtualDom::new(note_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "Research",                                            // the note type label
        "Immigration year unresolved",                         // the first paragraph
        "Need to confirm the immigration year for John Smith", // the second paragraph
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn language_tab_shows_the_translation_and_translator() {
    let mut vdom = VirtualDom::new(note_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "nb-NO",                                       // the translation's language tag
        "Må bekrefte innvandringsåret for John Smith", // the translated text
        "magne",                                       // the translator
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn references_list_every_record_that_uses_the_note() {
    let mut vdom = VirtualDom::new(note_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in ["John Smith", "Person", "Marriage", "Event", "E0007"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let mut vdom = VirtualDom::new(note_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains("Needs sources"), "tag name shown:\n{html}");
    assert!(html.contains("#e0884a"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
