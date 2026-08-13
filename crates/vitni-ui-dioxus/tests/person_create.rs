//! SSR assertions for the Person create pane (Phase 5 PR27/PR28): create renders inline in the detail
//! pane on the shared record form. The scalar identity fields render via the pure `person_record_fields`
//! fn (now wrapped in a Card — one `human-id` control, not two), and the name cites a citation through
//! the find-or-create picker (`person_name_citation_field`) — all SSR-testable without `AppCtx`.

use dioxus::prelude::*;
use vitni_ui::{Localizer, PersonDraft, PickerSelection, PickerState, ProvenanceDraft};
use vitni_ui_dioxus::components::{PickerCallbacks, PickerConfig, PickerOptions, RecordPicker};
use vitni_ui_dioxus::screens::{
    RecordEditState, create_record_header, person_name_citation_field, person_record_fields,
};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn noop_callbacks() -> PickerCallbacks {
    PickerCallbacks {
        onpick: Callback::new(|_: PickerSelection| {}),
        onclear: Callback::new(|()| {}),
        onnew: Callback::new(|_: String| {}),
    }
}

fn picker(label: String, name: &str) -> RecordPicker {
    RecordPicker {
        config: PickerConfig {
            label,
            name: name.to_owned(),
            entity_label: "citation".to_owned(),
            allow_new: true,
        },
        state: use_signal(PickerState::default),
        options: PickerOptions::Ready(Vec::new()),
        exclude: Vec::new(),
        callbacks: noop_callbacks(),
    }
}

fn create_pane_view() -> Element {
    let loc = loc();
    // A create edit state (edit mode from the start), with the given name pre-filled to prove the
    // fields are live inputs rather than read text.
    let record = RecordEditState::<PersonDraft> {
        editing: use_signal(|| true),
        seed: use_signal(PersonDraft::new),
        draft: use_signal(|| PersonDraft {
            given: "Ada".to_owned(),
            ..PersonDraft::new()
        }),
        prov: use_signal(ProvenanceDraft::default),
    };
    let citation = picker(loc.section_name_citation(), "name-citation");
    let source = picker(loc.field_label("source"), "citation-source");
    rsx! {
        {create_record_header(&loc.person_new_title(), &loc.record_draft_badge(), rsx! {})}
        {person_record_fields(&loc, record)}
        {person_name_citation_field(&loc, record.draft, &citation, &source)}
    }
}

#[test]
fn create_pane_shows_the_draft_header_carded_fields_and_citation_picker() {
    let mut vdom = VirtualDom::new(create_pane_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    for needle in [
        "New person",
        "draft · not saved",
        r#"class="card""#,
        r#"id="given""#,
        r#"id="surname""#,
        r#"id="prefix""#,
        r#"id="suffix""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    // The given-name input is seeded, proving the fields are live inputs (not read text).
    assert!(
        html.contains(r#"value="Ada""#),
        "the given-name input carries its value:\n{html}"
    );
    // Exactly one human-id control — the duplicate plain Input is gone (the DraftText is the single one).
    assert_eq!(
        html.matches(r#"id="human-id""#).count(),
        1,
        "there is a single human-id control:\n{html}"
    );
    // The name cites a citation through the find-or-create picker.
    assert!(
        html.contains("Citation for this name") && html.contains(r#"id="name-citation""#),
        "the citation picker is present:\n{html}"
    );
}
