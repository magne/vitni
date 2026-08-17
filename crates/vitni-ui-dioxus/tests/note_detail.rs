//! SSR assertions for the Note detail (Phase 5 PR27): the read-first Content record (id · type ·
//! content · language), its edit mode swapping in inputs plus the sticky-header Cancel/Save, the
//! language tab (primary language + translations), the references tab, and the tags panel.

use dioxus::prelude::*;
use vitni_app::{NoteType, TagRef, UsingKind};
use vitni_ui::{Localizer, NoteDetail, NoteDraft, ProvenanceDraft, RestrictionKind, TranslationVm, UsingRecordVm};
use vitni_ui_dioxus::components::TabItem;
use vitni_ui_dioxus::master_detail::DetailContainer;
use vitni_ui_dioxus::screens::{
    NoteEditForm, RecordActionLabels, RecordEditState, note_content_tab, note_language_tab, note_references_table,
    record_head_actions, restriction_display, tags_panel,
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
            assertion_id: "0190-text-assertion-id".to_owned(),
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
        {note_language_tab(&loc, &detail, use_callback(|_: NoteEditForm| {}), use_callback(|_: (String, String, bool)| {}))}
        {note_references_table(&loc, &detail.references)}
        {tags_panel(&loc, &detail.tags, use_callback(|_: (String, String)| {}))}
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

fn language_tab() -> Element {
    let loc = loc();
    let detail = sample();
    rsx! {
        {note_language_tab(&loc, &detail, use_callback(|_: NoteEditForm| {}), use_callback(|_: (String, String, bool)| {}))}
    }
}

#[test]
fn translation_row_is_edit_only_with_a_row_scoped_label_and_no_retract() {
    let html = render(language_tab);
    assert!(
        html.contains(r#"aria-label="Edit nb-NO""#),
        "the translation row Edit carries a row-scoped accessible name:\n{html}"
    );
    assert!(
        !html.contains(">Retract<") && !html.contains("aria-label=\"Retract"),
        "a translation has no Retract — removing one has no app verb:\n{html}"
    );
}

#[test]
fn the_translation_row_never_renders_the_text_assertion_id() {
    let html = render(language_tab);
    assert!(
        !html.contains("0190-text-assertion-id"),
        "the shared text-assertion id must never be rendered:\n{html}"
    );
}

fn references_only() -> Element {
    let loc = loc();
    let detail = sample();
    rsx! { {note_references_table(&loc, &detail.references)} }
}

#[test]
fn the_reverse_index_references_table_has_no_row_actions() {
    let html = render(references_only);
    assert!(html.contains("John Smith"), "the reference is listed:\n{html}");
    assert!(
        !html.contains("row-actions") && !html.contains(">Edit<") && !html.contains(">Detach<"),
        "the reverse-index references table offers no per-row actions:\n{html}"
    );
}

// ---- Restrictions (issue #315) --------------------------------------------------------------------

/// The note detail header as `note_detail` builds it: the restrictions in force as a read-only display
/// in the badge row — the header states them, it no longer changes them.
fn note_header(loc: &Localizer, detail: &NoteDetail) -> Element {
    let active = use_signal(|| 0_usize);
    rsx! {
        DetailContainer {
            title: detail.title.clone(),
            id_label: Some(detail.human_id.clone()),
            avatar: "🗒".to_owned(),
            extras: restriction_display(loc, &detail.restrictions),
            actions: rsx! {},
            tabs: Vec::<TabItem>::new(),
            active,
        }
    }
}

fn note_header_with_confidential() -> Element {
    let mut detail = sample();
    detail.restrictions = vec![RestrictionKind::Confidential];
    rsx! {
        {note_header(&loc(), &detail)}
    }
}

fn note_header_unrestricted() -> Element {
    rsx! {
        {note_header(&loc(), &sample())}
    }
}

/// The note's edit state over a confidential record, in `editing` mode.
fn confidential_state(editing: bool) -> RecordEditState<NoteDraft> {
    let mut detail = sample();
    detail.restrictions = vec![RestrictionKind::Confidential];
    let seed = NoteDraft::from_detail(&detail);
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

fn confidential_content_view_mode() -> Element {
    rsx! {
        {note_content_tab(&loc(), &sample(), confidential_state(false))}
    }
}

fn confidential_content_edit_mode() -> Element {
    rsx! {
        {note_content_tab(&loc(), &sample(), confidential_state(true))}
    }
}

/// A note draft differing from its committed seed in nothing but its restriction set.
fn note_restriction_change_only() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let seed = NoteDraft::from_detail(&sample());
    let mut draft = seed.clone();
    draft.restrictions = vec![RestrictionKind::Privacy];
    let record = RecordEditState {
        editing: use_signal(|| true),
        seed: use_signal(move || seed),
        draft: use_signal(move || draft),
        prov: use_signal(ProvenanceDraft::default),
    };
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (NoteDraft, ProvenanceDraft)| {}))}
    }
}

#[test]
fn the_header_states_only_the_restrictions_in_force() {
    let html = render(note_header_with_confidential);
    assert!(
        html.contains(r#"data-kind="confidential""#),
        "the set kind shows:\n{html}"
    );
    assert!(
        !html.contains(r#"data-kind="locked""#) && !html.contains(r#"data-kind="privacy""#),
        "an unset kind is not shown in the header at all:\n{html}"
    );
    assert!(
        !html.contains("<button"),
        "the header's restrictions are a display, not toggles (issue #315):\n{html}"
    );
}

#[test]
fn an_unrestricted_note_shows_no_header_restrictions() {
    let html = render(note_header_unrestricted);
    assert!(
        !html.contains("resn"),
        "an unrestricted record's header carries no restriction group:\n{html}"
    );
}

#[test]
fn the_content_card_states_every_restriction_in_view_mode() {
    let html = render(confidential_content_view_mode);
    assert!(html.contains(">Restrictions<"), "the card row is labelled:\n{html}");
    assert_eq!(
        html.matches("resn-static").count(),
        3,
        "all three kinds render, static, so entering edit mode reflows nothing:\n{html}"
    );
}

#[test]
fn the_content_card_toggles_restrictions_in_edit_mode() {
    let html = render(confidential_content_edit_mode);
    assert!(!html.contains("resn-static"), "edit mode offers live toggles:\n{html}");
    assert!(
        html.contains(r#"data-kind="confidential" aria-pressed="true""#),
        "the toggles are seeded from the draft:\n{html}"
    );
    assert_eq!(
        html.matches(r#"aria-pressed="true""#).count(),
        1,
        "only the record's own restriction is pressed:\n{html}"
    );
}

#[test]
fn a_restriction_change_alone_makes_the_note_savable() {
    let html = render(note_restriction_change_only);
    assert!(
        !html.contains("disabled"),
        "a restriction change alone enables Save:\n{html}"
    );
}
