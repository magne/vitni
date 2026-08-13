//! SSR assertions for the Citation create pane (Phase 5 PR27/PR28): the shared record frame in create
//! mode — a "draft · not saved" header with Cancel/Save in the sticky head — plus the required source
//! as a find-or-create picker (§7): Save is blocked until a source is picked, and "+ New" reveals the
//! inline new-source title field.

use dioxus::prelude::*;
use vitni_ui::{CitationDraft, Localizer, NewSourceFields, PickerSelection, PickerState, ProvenanceDraft, RecordLink};
use vitni_ui_dioxus::components::{Button, ButtonVariant, PickerCallbacks, PickerConfig, PickerOptions, RecordPicker};
use vitni_ui_dioxus::screens::{RecordEditState, citation_create_fields, create_record_header, record_edit_provenance};

fn noop_callbacks() -> PickerCallbacks {
    PickerCallbacks {
        onpick: Callback::new(|_: PickerSelection| {}),
        onclear: Callback::new(|()| {}),
        onnew: Callback::new(|_: String| {}),
    }
}

fn view(seed: CitationDraft) -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let record = RecordEditState::<CitationDraft> {
        editing: use_signal(|| true),
        seed: use_signal(CitationDraft::new),
        draft: use_signal(move || seed),
        prov: use_signal(ProvenanceDraft::default),
    };
    let source = RecordPicker {
        config: PickerConfig {
            label: loc.field_label("source"),
            name: "citation-source".to_owned(),
            entity_label: "source".to_owned(),
            allow_new: true,
        },
        state: use_signal(PickerState::default),
        options: PickerOptions::Ready(Vec::new()),
        exclude: Vec::new(),
        callbacks: noop_callbacks(),
    };
    let can_save = record.can_save();
    let actions = rsx! {
        Button { label: loc.action_label("cancel"), variant: ButtonVariant::Ghost, small: true, onclick: move |_| {} }
        Button { label: loc.action_label("save"), variant: ButtonVariant::Primary, small: true, disabled: !can_save, onclick: move |_| {} }
    };
    rsx! {
        {create_record_header(&loc.citation_new_title(), &loc.record_draft_badge(), actions)}
        {citation_create_fields(&loc, record.draft, &source)}
        {record_edit_provenance(&loc, record)}
    }
}

fn empty_view() -> Element {
    view(CitationDraft::new())
}

fn sourced_view() -> Element {
    view(CitationDraft {
        source: RecordLink::Existing(PickerSelection {
            human_id: "S0001".to_owned(),
            title: "Baptism register".to_owned(),
        }),
        ..CitationDraft::new()
    })
}

fn new_source_view() -> Element {
    view(CitationDraft {
        source: RecordLink::New(NewSourceFields::default()),
        ..CitationDraft::new()
    })
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn a_missing_source_blocks_save_and_shows_the_picker() {
    let html = render(empty_view);
    for needle in [
        "New citation",
        "draft · not saved",
        r#"id="citation-source""#,
        r#"for="citation-date""#,
        r#"id="citation-page""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    assert!(html.contains("disabled"), "Save blocked without a source:\n{html}");
}

#[test]
fn a_picked_source_enables_save() {
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
        html.contains(r#"class="draft-card""#),
        "the inline new source renders in a draft card:\n{html}"
    );
    assert!(
        !html.contains("disabled"),
        "a new source makes the draft valid + dirty:\n{html}"
    );
}
