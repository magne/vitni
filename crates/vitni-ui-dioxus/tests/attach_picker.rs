//! SSR assertions for the side-panel attach/link form body (Phase 5 PR28): every collection-row link
//! side panel (person/event/family/citation/media/source/repository/dna attach + link forms) is a thin
//! wrapper over the shared [`attach_picker_form`], so exercising that body proves each site renders an
//! existing-only record picker (a search-and-select control), never a bare free-text `human_id` input.
//! Each per-screen test builds the picker exactly as its screen does to document the conversion.

use dioxus::prelude::*;
use vitni_ui::{Localizer, PickerSelection, PickerState, ProvenanceDraft, RowVm};
use vitni_ui_dioxus::components::{PickerCallbacks, PickerConfig, PickerOptions, RecordPicker};
use vitni_ui_dioxus::screens::attach_picker_form;

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

/// Builds an existing-only picker the way every side panel does (`allow_new: false`), optionally seeded
/// with a collapsed selection (a picked record) via `selected`.
fn picker(name: &str, entity: &str, selected: Option<PickerSelection>) -> RecordPicker {
    RecordPicker {
        config: PickerConfig {
            label: "Field".to_owned(),
            name: name.to_owned(),
            entity_label: entity.to_owned(),
            allow_new: false,
        },
        state: use_signal(move || PickerState {
            query: String::new(),
            open: false,
            selection: selected,
        }),
        options: PickerOptions::Ready(rows()),
        exclude: Vec::new(),
        callbacks: PickerCallbacks {
            onpick: Callback::new(|_: PickerSelection| {}),
            onclear: Callback::new(|()| {}),
            onnew: Callback::new(|_: String| {}),
        },
    }
}

fn body(name: &str, entity: &str, selected: Option<PickerSelection>, extra: Element) -> Element {
    let loc = loc();
    let prov = use_signal(ProvenanceDraft::default);
    let picker = picker(name, entity, selected);
    attach_picker_form(&loc, &picker, extra, prov, Callback::new(|()| {}))
}

fn unpicked_view() -> Element {
    body("note", "note", None, rsx! {})
}

fn picked_view() -> Element {
    body(
        "note",
        "note",
        Some(PickerSelection {
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
        None,
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
