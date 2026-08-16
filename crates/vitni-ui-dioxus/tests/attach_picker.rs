//! SSR assertions for the side-panel attach/link form body (issue #314): every collection-row link
//! side panel (person/event/family/citation/media/source/repository/dna attach + link forms) is a thin
//! wrapper over the shared [`attach_link_form`], so exercising that body proves each site renders a
//! find-or-create record picker — a search-and-select control that also offers "+ New …" wherever the
//! category supports inline creation — never a bare free-text `human_id` input. Each per-screen test
//! builds the picker exactly as its screen does (via [`use_attach_picker`]) to document the conversion.
//!
//! SSR cannot click "+ New" (it never reaches the picker's `PickerSearch` component, which owns the
//! click), so the "+ New …" tests seed [`AttachLink::link`] directly and assert the rendered branch —
//! the same trick `tests/provenance_block.rs`'s new-citation-card test uses.

use dioxus::prelude::*;
use vitni_ui::{
    Localizer, NewNoteFields, NewRecordDraft, PickerSelection, PickerState, ProvenanceDraft, RecordLink, RowVm,
};
use vitni_ui_dioxus::components::{PickerCallbacks, PickerConfig, PickerOptions, RecordPicker};
use vitni_ui_dioxus::screens::{AttachLink, AttachPicker, attach_link_form};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn rows() -> Vec<RowVm> {
    vec![
        RowVm {
            id: "R0001".to_owned(),
            title: "First record".to_owned(),
            ..RowVm::default()
        },
        RowVm {
            id: "R0002".to_owned(),
            title: "Second record".to_owned(),
            ..RowVm::default()
        },
    ]
}

/// Builds a find-or-create attach picker the way every side panel does (`allow_new: true`), seeded with
/// `link` — unset, an existing pick, or a "+ New …" draft. `open` controls whether the floating result
/// list (and, when the link is unset, its trailing "+ New …" row) is rendered. `onclear` mirrors
/// [`use_attach_picker`]'s real wiring (flips `link` back to [`RecordLink::Empty`]) so the discard test
/// can drive it directly; `onpick`/`onnew` stay no-ops, since no test here simulates a click through them.
fn attach(name: &str, entity: &str, link_value: RecordLink<NewRecordDraft>, open: bool) -> AttachPicker {
    let selection = match &link_value {
        RecordLink::Existing(selection) => Some(selection.clone()),
        RecordLink::Empty | RecordLink::New(_) => None,
    };
    let state = use_signal(move || PickerState {
        query: String::new(),
        open,
        selection: selection.clone(),
    });
    let mut link = use_signal(move || link_value.clone());
    let picker = RecordPicker {
        config: PickerConfig {
            label: "Field".to_owned(),
            name: name.to_owned(),
            entity_label: entity.to_owned(),
            allow_new: true,
        },
        state,
        options: PickerOptions::Ready(rows()),
        exclude: Vec::new(),
        callbacks: PickerCallbacks {
            onpick: Callback::new(|_: PickerSelection| {}),
            onclear: Callback::new(move |()| link.set(RecordLink::Empty)),
            onnew: Callback::new(|_: String| {}),
        },
    };
    AttachPicker {
        picker,
        link: AttachLink {
            link,
            state,
            error: use_signal(|| None::<String>),
            saving: use_signal(|| false),
        },
    }
}

fn body(name: &str, entity: &str, link_value: RecordLink<NewRecordDraft>, extra: Element) -> Element {
    let loc = loc();
    let prov = use_signal(ProvenanceDraft::default);
    let attach = attach(name, entity, link_value, false);
    attach_link_form(&loc, &attach, extra, prov, Callback::new(|()| {}))
}

fn unpicked_view() -> Element {
    body("note", "note", RecordLink::Empty, rsx! {})
}

fn picked_view() -> Element {
    body(
        "note",
        "note",
        RecordLink::Existing(PickerSelection {
            human_id: "N0007".to_owned(),
            title: "Baptism note".to_owned(),
        }),
        rsx! {},
    )
}

fn extra_view() -> Element {
    body(
        "participant",
        "person",
        RecordLink::Empty,
        rsx! {
            div { class: "field", label { "Role" } select { option { "Witness" } } }
        },
    )
}

#[test]
fn an_unpicked_form_shows_a_search_picker_and_disables_save() {
    let html = render(unpicked_view);
    assert!(
        html.contains(r#"placeholder="Find note…""#),
        "the link field is a search picker, not a free-text id input:\n{html}"
    );
    assert!(
        html.contains(r#"id="note""#),
        "the picker input carries the field name:\n{html}"
    );
    assert!(
        html.contains("disabled") && html.contains(">Save<"),
        "Save is blocked until a record is picked:\n{html}"
    );
    assert!(
        html.contains("Confidence"),
        "the provenance block renders below the picker:\n{html}"
    );
}

#[test]
fn a_picked_form_collapses_to_a_value_chip_and_enables_save() {
    let html = render(picked_view);
    assert!(
        html.contains(r#"class="picker-value""#) && html.contains("Baptism note") && html.contains("N0007"),
        "the picked record collapses to a labelled chip:\n{html}"
    );
    assert!(
        !html.contains("disabled"),
        "Save is enabled once a record is picked:\n{html}"
    );
    // The note link collapsed to a chip: its own search input is gone. (The provenance block below
    // carries its own citation picker, so a bare "Find" match no longer isolates the note picker.)
    assert!(
        !html.contains(r#"placeholder="Find note…""#),
        "no note search input while a record is picked:\n{html}"
    );
}

#[test]
fn an_extra_field_renders_between_the_picker_and_provenance() {
    let html = render(extra_view);
    assert!(
        html.contains(r#"placeholder="Find person…""#),
        "the participant field is a person picker:\n{html}"
    );
    assert!(
        html.contains("Role") && html.contains("Witness"),
        "the extra role select renders:\n{html}"
    );
}

fn open_unpicked_view() -> Element {
    let loc = loc();
    let prov = use_signal(ProvenanceDraft::default);
    let attach = attach("note", "note", RecordLink::Empty, true);
    attach_link_form(&loc, &attach, rsx! {}, prov, Callback::new(|()| {}))
}

#[test]
fn an_open_unpicked_picker_offers_a_new_row() {
    let html = render(open_unpicked_view);
    assert!(
        html.contains(r#"class="picker-new""#),
        "an existing-only picker never offered this; a find-or-create one does:\n{html}"
    );
}

fn new_note_view() -> Element {
    body(
        "note",
        "note",
        RecordLink::New(NewRecordDraft::Note(NewNoteFields {
            text: "A research note".to_owned(),
        })),
        rsx! {},
    )
}

#[test]
fn choosing_new_replaces_the_picker_with_a_nested_draft_card() {
    let html = render(new_note_view);
    assert!(
        html.contains(r#"class="draft-card""#),
        "the picker is replaced by the nested draft card:\n{html}"
    );
    assert!(
        !html.contains(r#"placeholder="Find note…""#),
        "the search picker is gone while drafting:\n{html}"
    );
}

fn seeded_new_view() -> Element {
    let Some(draft) = NewRecordDraft::seed(vitni_ui::Category::Notes, "Ellis Island") else {
        return rsx! {};
    };
    body("note", "note", RecordLink::New(draft), rsx! {})
}

#[test]
fn the_nested_card_seeds_its_field_with_the_typed_query() {
    let html = render(seeded_new_view);
    assert!(
        html.contains("Ellis Island"),
        "the typed query seeds the new note's own field:\n{html}"
    );
}

fn discarded_view() -> Element {
    let loc = loc();
    let prov = use_signal(ProvenanceDraft::default);
    let attach = attach(
        "note",
        "note",
        RecordLink::New(NewRecordDraft::Note(NewNoteFields::default())),
        false,
    );
    // SSR cannot click the card's discard button, so drive the exact callback it is wired to
    // (`attach_link_field` binds `NewRecordCard::onclose` to `attach.picker.callbacks.onclear`) before
    // the one render this test takes.
    use_hook(move || {
        attach.picker.callbacks.onclear.call(());
    });
    attach_link_form(&loc, &attach, rsx! {}, prov, Callback::new(|()| {}))
}

#[test]
fn discarding_the_card_returns_the_form_to_the_search_picker() {
    let html = render(discarded_view);
    assert!(
        !html.contains(r#"class="draft-card""#),
        "the card is gone once discarded:\n{html}"
    );
    assert!(
        html.contains(r#"placeholder="Find note…""#),
        "the form falls back to the search picker:\n{html}"
    );
}

#[test]
fn save_is_disabled_while_the_nested_card_is_incomplete() {
    let html = render(new_note_view_blank);
    assert!(html.contains(r#"class="draft-card""#), "the card renders:\n{html}");
    assert!(
        html.contains("disabled") && html.contains(">Save<"),
        "Save stays blocked while the draft has not validated:\n{html}"
    );
}

fn new_note_view_blank() -> Element {
    body(
        "note",
        "note",
        RecordLink::New(NewRecordDraft::Note(NewNoteFields::default())),
        rsx! {},
    )
}
