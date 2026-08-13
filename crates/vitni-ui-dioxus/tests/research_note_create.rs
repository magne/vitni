//! SSR assertions for the `ResearchNote` create pane (issue #194): the shared record frame in create
//! mode — a "draft · not saved" header with the id/title/argument/language fields (the argument a
//! textarea), and the subject editor that gates the Save until the draft names at least one subject
//! (ADR 0028 §2).

use dioxus::prelude::*;
use vitni_ui::{Category, Localizer, ProvenanceDraft, RecordDraft, ResearchNoteDraft, SubjectVm};
use vitni_ui_dioxus::components::{Button, ButtonVariant};
use vitni_ui_dioxus::screens::{
    RecordEditState, create_record_header, record_edit_provenance, research_note_draft_subjects,
    research_note_record_fields,
};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// A create draft naming one person subject, the shape the reverse-lookup tab's Add pre-seeds.
fn seeded_draft() -> ResearchNoteDraft {
    let mut draft = ResearchNoteDraft {
        title: "Same person as the 1865 census?".to_owned(),
        ..ResearchNoteDraft::new()
    };
    draft.add_subject(SubjectVm {
        category: Category::People,
        human_id: "I0042".to_owned(),
        id: String::new(),
        kind_label: "Person".to_owned(),
    });
    draft
}

fn state(draft: ResearchNoteDraft) -> RecordEditState<ResearchNoteDraft> {
    RecordEditState::<ResearchNoteDraft> {
        editing: use_signal(|| true),
        seed: use_signal(ResearchNoteDraft::new),
        draft: use_signal(move || draft),
        prov: use_signal(ProvenanceDraft::default),
    }
}

fn empty_create_view() -> Element {
    let loc = loc();
    let record = state(ResearchNoteDraft::new());
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_label("save"), variant: ButtonVariant::Primary, small: true, disabled: !record.can_save(), onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.research_note_new_title(), &loc.record_draft_badge(), actions)}
        {research_note_record_fields(&loc, record)}
        {research_note_draft_subjects(&loc, record)}
        {record_edit_provenance(&loc, record)}
    }
}

fn seeded_create_view() -> Element {
    let loc = loc();
    let record = state(seeded_draft());
    rsx! {
        {research_note_draft_subjects(&loc, record)}
    }
}

#[test]
fn create_pane_shows_the_draft_badge_and_labelled_fields() {
    let html = render(empty_create_view);
    for needle in [
        "New research note",
        "draft · not saved",
        "Title",
        "Argument",
        "Language",
        r#"id="research-note-id""#,
        r#"id="research-note-title""#,
        r#"id="research-note-language""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert!(
        html.contains("<textarea"),
        "the argument is a textarea in create mode:\n{html}"
    );
}

#[test]
fn create_pane_save_is_disabled_until_a_subject_is_named() {
    let html = render(empty_create_view);
    assert!(
        html.contains("disabled"),
        "Save disabled while no subject is named:\n{html}"
    );
    assert!(
        html.contains("A research note must name at least one subject."),
        "the create form says why it cannot save yet:\n{html}"
    );
    assert!(
        !ResearchNoteDraft::new().is_valid(),
        "the draft itself is the Save gate"
    );
}

#[test]
fn a_seeded_subject_shows_as_a_removable_chip_and_clears_the_validation_note() {
    let html = render(seeded_create_view);
    assert!(html.contains("I0042"), "the seeded subject is listed:\n{html}");
    assert!(html.contains("Person"), "with its localized kind:\n{html}");
    assert!(
        html.contains("Remove I0042"),
        "the chip carries a row-scoped remove control:\n{html}"
    );
    assert!(
        !html.contains("A research note must name at least one subject."),
        "the validation note is gone once a subject is named:\n{html}"
    );
}
