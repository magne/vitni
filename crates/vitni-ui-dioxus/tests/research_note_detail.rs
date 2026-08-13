//! SSR assertions for the `ResearchNote` detail (issue #194): the read-first Content record (argument ·
//! language) with the id/title read-only because the aggregate has no rename or title-set verb, the
//! Subjects tab's linked rows with a per-row Remove, the tags panel, and the reverse-lookup table the
//! four subject screens share.

use dioxus::prelude::*;
use vitni_app::TagRef;
use vitni_ui::{Category, Localizer, ProvenanceDraft, ResearchNoteDetail, ResearchNoteDraft, RowVm, SubjectVm};
use vitni_ui_dioxus::screens::{
    RecordActionLabels, RecordEditState, ResearchNoteEditForm, record_head_actions, research_note_content_tab,
    research_note_subjects_table, research_notes_table, tags_panel,
};
use vitni_ui_dioxus::shell::nav_state::NavState;

/// A representative research note: an argument about a person and a place, carrying one tag.
fn sample() -> ResearchNoteDetail {
    ResearchNoteDetail {
        human_id: "A0001".to_owned(),
        id: "0190-research-note-id".to_owned(),
        title: "Same person as the 1865 census entry?".to_owned(),
        body: Some("The parish register and the census agree on the birth year.".to_owned()),
        language: Some("en".to_owned()),
        subjects: vec![
            SubjectVm {
                category: Category::People,
                human_id: "I0042".to_owned(),
                id: "0190-person-42".to_owned(),
                kind_label: "Person".to_owned(),
            },
            SubjectVm {
                category: Category::Places,
                human_id: "P0007".to_owned(),
                id: "0190-place-7".to_owned(),
                kind_label: "Place".to_owned(),
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

fn state(editing: bool) -> RecordEditState<ResearchNoteDraft> {
    let seed = ResearchNoteDraft::from_detail(&sample());
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

fn view_mode() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(false);
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (ResearchNoteDraft, ProvenanceDraft)| {}))}
        {research_note_content_tab(&loc, record)}
        {tags_panel(&loc, &sample().tags, use_signal(|| None::<ResearchNoteEditForm>), ResearchNoteEditForm::Tag, use_callback(|_: String| {}))}
    }
}

fn edit_mode() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let record = state(true);
    rsx! {
        {record_head_actions(&labels, record, rsx! {}, use_callback(|_: (ResearchNoteDraft, ProvenanceDraft)| {}))}
        {research_note_content_tab(&loc, record)}
    }
}

#[test]
fn content_is_read_first_with_an_edit_button_and_no_inputs() {
    let html = render(view_mode);
    assert!(html.contains(">Edit<"), "view mode offers Edit:\n{html}");
    assert!(
        !html.contains("<input") && !html.contains("<textarea"),
        "no live inputs in view mode:\n{html}"
    );
    assert!(
        html.contains("The parish register and the census agree on the birth year."),
        "the argument is shown:\n{html}"
    );
}

#[test]
fn edit_mode_swaps_in_the_argument_and_language_inputs() {
    let html = render(edit_mode);
    assert!(
        html.contains("<textarea"),
        "the argument becomes a textarea in edit mode:\n{html}"
    );
    assert!(
        html.contains(">Cancel<") && html.contains(">Save<"),
        "Cancel/Save in the header:\n{html}"
    );
    assert!(
        html.contains(r#"id="research-note-language""#),
        "the language input is present:\n{html}"
    );
}

#[test]
fn the_id_and_title_stay_read_only_on_a_saved_note() {
    let html = render(edit_mode);
    assert!(
        !html.contains(r#"id="research-note-id""#),
        "there is no rename verb, so edit mode offers no id input:\n{html}"
    );
    assert!(
        !html.contains(r#"id="research-note-title""#),
        "there is no title-set verb, so edit mode offers no title input:\n{html}"
    );
    assert!(
        html.contains("A0001") && html.contains("Same person as the 1865 census entry?"),
        "both are still shown as read boxes:\n{html}"
    );
}

fn subjects_tab() -> Element {
    use_context_provider(NavState::new);
    let loc = loc();
    rsx! {
        {research_note_subjects_table(&loc, &sample().subjects, use_callback(|_: SubjectVm| {}))}
    }
}

#[test]
fn subjects_list_every_named_record_with_its_kind_and_a_remove() {
    let html = render(subjects_tab);
    for needle in ["I0042", "Person", "P0007", "Place"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert!(
        html.contains(r#"aria-label="Remove I0042""#),
        "each subject row carries a row-scoped Remove:\n{html}"
    );
    assert!(
        !html.contains("0190-person-42") && !html.contains("0190-place-7"),
        "a subject's aggregate id must never be rendered:\n{html}"
    );
}

#[test]
fn tags_show_name_and_colour_never_the_id() {
    let html = render(view_mode);
    assert!(html.contains("Needs sources"), "tag name shown:\n{html}");
    assert!(html.contains("#e0884a"), "tag colour dot shown:\n{html}");
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}

fn reverse_rows() -> Element {
    // `RecordLink` resolves `NavState` from context; the bare SSR harness must supply it.
    use_context_provider(NavState::new);
    let loc = loc();
    let rows = vec![RowVm {
        id: "A0001".to_owned(),
        title: "Same person as the 1865 census entry?".to_owned(),
        subtitle: Some("about I0042".to_owned()),
        avatar: Some("🧾".to_owned()),
        ..RowVm::default()
    }];
    rsx! { {research_notes_table(&loc, &rows)} }
}

fn reverse_empty() -> Element {
    let loc = loc();
    rsx! { {research_notes_table(&loc, &[])} }
}

#[test]
fn the_reverse_lookup_table_links_each_argument_by_its_id() {
    let html = render(reverse_rows);
    assert!(
        html.contains("Same person as the 1865 census entry?"),
        "the argument's title is the link text:\n{html}"
    );
    assert!(html.contains("A0001"), "its id is shown:\n{html}");
}

#[test]
fn the_reverse_lookup_table_has_an_empty_state() {
    let html = render(reverse_empty);
    assert!(
        html.contains("Nothing here yet."),
        "a record with no arguments about it shows the shared empty state:\n{html}"
    );
}
