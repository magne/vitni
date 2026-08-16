//! SSR assertions for [`NewRecordCard`] (issue #314): the nested "+ New …" draft card an attach
//! picker's "+ New …" row opens. SSR cannot click "+ New" (it never reaches the picker's `PickerSearch`
//! component, which owns the click), so every test here seeds `RecordLink::New(..)` directly and
//! asserts the rendered branch — the same trick `provenance_block.rs`'s new-citation-card test uses.
//! One test per supported category, plus an anti-drift test that every one of the eight renders at
//! least one field. The three categories with a required *extra* field beyond their primary one (Media,
//! Event, Citation) are also driven through [`attach_link_form`] so the Save-disabled wiring
//! ([`link_is_savable`]) is exercised, not just the card's own fields.

use dioxus::prelude::*;
use vitni_ui::{
    Localizer, NewCitationFields, NewEventFields, NewMediaFields, NewNoteFields, NewPersonFields, NewPlaceFields,
    NewRecordDraft, NewRepositoryFields, NewSourceFields, PickerSelection, PickerState, ProvenanceDraft, RecordLink,
};
use vitni_ui_dioxus::components::{NewRecordCard, PickerCallbacks, PickerConfig, PickerOptions, RecordPicker};
use vitni_ui_dioxus::screens::{AttachLink, AttachPicker, attach_link_form};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// Mounts [`NewRecordCard`] over `link_value`, matching how [`super::attach_link_field`] conditionally
/// mounts it once the attach picker's link holds a draft.
fn card(link_value: &RecordLink<NewRecordDraft>) -> Element {
    let link_value = link_value.clone();
    let link = use_signal(move || link_value.clone());
    let error = use_signal(|| None::<String>);
    rsx! {
        NewRecordCard { link, error, onclose: Callback::new(|()| {}) }
    }
}

fn person_view() -> Element {
    card(&RecordLink::New(NewRecordDraft::Person(NewPersonFields {
        given: "Ada".to_owned(),
        surname: "Lovelace".to_owned(),
    })))
}

fn place_view() -> Element {
    card(&RecordLink::New(NewRecordDraft::Place(NewPlaceFields {
        place_type: vitni_app::PlaceType::City,
        name: "Ellis Island".to_owned(),
    })))
}

fn source_view() -> Element {
    card(&RecordLink::New(NewRecordDraft::Source(NewSourceFields {
        title: "Baptism register".to_owned(),
    })))
}

fn citation_view() -> Element {
    card(&RecordLink::New(NewRecordDraft::Citation(NewCitationFields {
        source: RecordLink::Empty,
        page: "p. 14".to_owned(),
    })))
}

fn note_view() -> Element {
    card(&RecordLink::New(NewRecordDraft::Note(NewNoteFields {
        text: "A research note".to_owned(),
    })))
}

fn media_view() -> Element {
    card(&RecordLink::New(NewRecordDraft::Media(NewMediaFields {
        file_path: "/photos/ada.jpg".to_owned(),
    })))
}

fn event_view() -> Element {
    card(&RecordLink::New(NewRecordDraft::Event(NewEventFields::default())))
}

fn repository_view() -> Element {
    card(&RecordLink::New(NewRecordDraft::Repository(NewRepositoryFields {
        name: "National Archives".to_owned(),
    })))
}

#[test]
fn person_card_renders_given_and_surname_inputs() {
    let html = render(person_view);
    assert!(html.contains("New person"), "the card title:\n{html}");
    assert!(html.contains(r#"class="badge draft""#), "the draft badge:\n{html}");
    assert!(
        html.contains(r#"id="new-record-person-given""#),
        "the given input:\n{html}"
    );
    assert!(
        html.contains(r#"id="new-record-person-surname""#),
        "the surname input:\n{html}"
    );
}

#[test]
fn place_card_renders_a_type_select_and_a_name_input() {
    let html = render(place_view);
    assert!(html.contains("New place"), "the card title:\n{html}");
    assert!(
        html.contains(r#"id="new-record-place-type""#),
        "the type select:\n{html}"
    );
    assert!(
        html.contains(r#"id="new-record-place-name""#),
        "the name input:\n{html}"
    );
}

#[test]
fn source_card_renders_a_title_input() {
    let html = render(source_view);
    assert!(html.contains("New source"), "the card title:\n{html}");
    assert!(
        html.contains(r#"id="new-record-source-title""#),
        "the title input:\n{html}"
    );
}

#[test]
fn citation_card_renders_a_source_picker_and_a_page_input() {
    let html = render(citation_view);
    assert!(html.contains("New citation"), "the card title:\n{html}");
    assert!(
        html.contains(r#"placeholder="Find source…""#),
        "the nested source picker (agrees with provenance_block.rs's new-citation card):\n{html}"
    );
    assert!(
        html.contains(r#"id="new-record-citation-page""#),
        "the page input:\n{html}"
    );
}

#[test]
fn note_card_renders_a_content_textarea() {
    let html = render(note_view);
    assert!(html.contains("New note"), "the card title:\n{html}");
    assert!(
        html.contains(r#"id="new-record-note-text""#),
        "the content textarea:\n{html}"
    );
}

#[test]
fn media_card_renders_a_file_path_input() {
    let html = render(media_view);
    assert!(html.contains("New media"), "the card title:\n{html}");
    assert!(
        html.contains(r#"id="new-record-media-file-path""#),
        "the file-path input:\n{html}"
    );
}

#[test]
fn event_card_offers_a_type_select_with_no_preselection() {
    let html = render(event_view);
    assert!(html.contains("New event"), "the card title:\n{html}");
    assert!(
        html.contains(r#"id="new-record-event-type""#),
        "the type select:\n{html}"
    );
    assert!(
        html.contains(r#"id="new-record-event-description""#),
        "the description input:\n{html}"
    );
    assert!(
        html.contains(r#"<option value="" selected=true>—</option>"#),
        "no event type is pre-chosen — a new event has deliberately no default type:\n{html}"
    );
}

#[test]
fn repository_card_renders_a_name_input() {
    let html = render(repository_view);
    assert!(html.contains("New repository"), "the card title:\n{html}");
    assert!(
        html.contains(r#"id="new-record-repository-name""#),
        "the name input:\n{html}"
    );
}

/// A category name paired with the fn rendering its card, for the anti-drift loop below.
type NamedView = (&'static str, fn() -> Element);

#[test]
fn every_supported_category_renders_a_card_with_at_least_one_field() {
    let views: [NamedView; 8] = [
        ("person", person_view),
        ("place", place_view),
        ("source", source_view),
        ("citation", citation_view),
        ("note", note_view),
        ("media", media_view),
        ("event", event_view),
        ("repository", repository_view),
    ];
    for (name, view) in views {
        let html = render(view);
        assert!(
            html.contains(r#"class="field""#),
            "{name}'s card renders at least one field:\n{html}"
        );
        assert!(
            html.contains(r#"class="draft-card""#),
            "{name}'s card renders the draft-card shell:\n{html}"
        );
    }
}

fn existing_link_view() -> Element {
    card(&RecordLink::Existing(PickerSelection {
        human_id: "N0007".to_owned(),
        title: "Baptism note".to_owned(),
    }))
}

fn empty_link_view() -> Element {
    card(&RecordLink::Empty)
}

#[test]
fn the_card_renders_nothing_once_the_link_is_no_longer_new() {
    for view in [existing_link_view, empty_link_view] {
        let html = render(view);
        assert!(
            !html.contains(r#"class="draft-card""#),
            "no card once the link resolves:\n{html}"
        );
    }
}

fn failed_create_view() -> Element {
    let link = use_signal(|| {
        RecordLink::New(NewRecordDraft::Note(NewNoteFields {
            text: "A research note".to_owned(),
        }))
    });
    let error = use_signal(|| Some("Could not save the note".to_owned()));
    rsx! {
        NewRecordCard { link, error, onclose: Callback::new(|()| {}) }
    }
}

#[test]
fn a_create_failure_renders_inside_the_card() {
    let html = render(failed_create_view);
    assert!(
        html.contains(r#"role="alert""#) && html.contains("Could not save the note"),
        "the localized create failure renders inside the card, not as a shell notice:\n{html}"
    );
    assert!(
        html.contains(r#"id="new-record-note-text""#),
        "every typed character survives a failed create:\n{html}"
    );
}

/// Builds a minimal [`AttachPicker`] over `link_value`, sharing one [`PickerState`] between the picker
/// and the link — the same wiring [`super::use_attach_picker`] gives a real call site — so
/// [`attach_link_form`]'s Save-disabled rule can be exercised without a live workspace.
fn attach_over(link_value: RecordLink<NewRecordDraft>) -> AttachPicker {
    let state = use_signal(PickerState::default);
    let picker = RecordPicker {
        config: PickerConfig {
            label: "Field".to_owned(),
            name: "field".to_owned(),
            entity_label: "record".to_owned(),
            allow_new: true,
        },
        state,
        options: PickerOptions::Ready(Vec::new()),
        exclude: Vec::new(),
        callbacks: PickerCallbacks {
            onpick: Callback::new(|_: PickerSelection| {}),
            onclear: Callback::new(|()| {}),
            onnew: Callback::new(|_: String| {}),
        },
    };
    AttachPicker {
        picker,
        link: AttachLink {
            link: use_signal(|| link_value),
            state,
            error: use_signal(|| None::<String>),
            saving: use_signal(|| false),
        },
    }
}

fn media_form_view() -> Element {
    let loc = loc();
    let attach = attach_over(RecordLink::New(NewRecordDraft::Media(NewMediaFields::default())));
    let prov = use_signal(ProvenanceDraft::default);
    attach_link_form(&loc, &attach, rsx! {}, prov, Callback::new(|()| {}))
}

#[test]
fn media_form_disables_save_while_the_path_is_blank() {
    let html = render(media_form_view);
    assert!(
        html.contains(r#"id="new-record-media-file-path""#),
        "the path field renders:\n{html}"
    );
    assert!(
        html.contains("disabled") && html.contains(">Save<"),
        "Save is blocked while blank:\n{html}"
    );
}

fn event_form_view() -> Element {
    let loc = loc();
    let attach = attach_over(RecordLink::New(NewRecordDraft::Event(NewEventFields::default())));
    let prov = use_signal(ProvenanceDraft::default);
    attach_link_form(&loc, &attach, rsx! {}, prov, Callback::new(|()| {}))
}

#[test]
fn event_form_disables_save_with_no_type_chosen() {
    let html = render(event_form_view);
    assert!(
        html.contains(r#"<option value="" selected=true>—</option>"#),
        "no event type is pre-chosen:\n{html}"
    );
    assert!(
        html.contains("disabled") && html.contains(">Save<"),
        "Save is blocked with no type chosen:\n{html}"
    );
}

fn citation_form_view() -> Element {
    let loc = loc();
    let attach = attach_over(RecordLink::New(NewRecordDraft::Citation(NewCitationFields {
        source: RecordLink::Empty,
        page: "p. 14".to_owned(),
    })));
    let prov = use_signal(ProvenanceDraft::default);
    attach_link_form(&loc, &attach, rsx! {}, prov, Callback::new(|()| {}))
}

#[test]
fn citation_form_disables_save_while_the_source_link_is_empty() {
    let html = render(citation_form_view);
    assert!(
        html.contains(r#"placeholder="Find source…""#),
        "the nested source picker renders:\n{html}"
    );
    assert!(
        html.contains(r#"id="new-record-citation-page""#),
        "the page input renders:\n{html}"
    );
    assert!(
        html.contains("disabled") && html.contains(">Save<"),
        "Save is blocked while the required source link is unset:\n{html}"
    );
}
