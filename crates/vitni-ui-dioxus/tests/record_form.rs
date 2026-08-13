//! SSR assertions for the shared record-form pieces (Phase 5 PR27): the no-reflow `DraftText` /
//! `DraftSelect` fields, the sticky-header `record_head_actions`, the dirty-gated
//! `record_edit_provenance`, and the create-frame header. Pure render-and-inspect — no window, no
//! workspace — over `TagDraft` as a representative record draft.

use dioxus::prelude::*;
use vitni_ui::{Localizer, ProvenanceDraft, TagDraft};
use vitni_ui_dioxus::components::{DraftSelect, DraftText, SelectChoice};
use vitni_ui_dioxus::screens::{
    RecordActionLabels, RecordEditState, create_record_frame, create_record_header, record_edit_provenance,
    record_head_actions,
};

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

// ---- DraftText -----------------------------------------------------------------------------------

fn draft_text_view() -> Element {
    rsx! {
        DraftText {
            label: "Given name".to_owned(),
            name: "given".to_owned(),
            editing: false,
            value: "John".to_owned(),
            original: "John".to_owned(),
            reset_label: "Reset Given name to original value".to_owned(),
            oninput: move |_: String| {},
            onreset: move |()| {},
        }
    }
}

fn draft_text_edit_unmodified() -> Element {
    rsx! {
        DraftText {
            label: "Given name".to_owned(),
            name: "given".to_owned(),
            editing: true,
            value: "John".to_owned(),
            original: "John".to_owned(),
            reset_label: "Reset Given name to original value".to_owned(),
            oninput: move |_: String| {},
            onreset: move |()| {},
        }
    }
}

fn draft_text_edit_modified() -> Element {
    rsx! {
        DraftText {
            label: "Given name".to_owned(),
            name: "given".to_owned(),
            editing: true,
            value: "Jane".to_owned(),
            original: "John".to_owned(),
            reset_label: "Reset Given name to original value".to_owned(),
            oninput: move |_: String| {},
            onreset: move |()| {},
        }
    }
}

#[test]
fn draft_text_view_mode_is_a_read_box_without_an_input() {
    let html = render(draft_text_view);
    assert!(html.contains(r#"class="val""#), "view mode renders a read box:\n{html}");
    assert!(html.contains("John"), "the value is shown as read text:\n{html}");
    assert!(!html.contains("<input"), "view mode has no live input:\n{html}");
}

#[test]
fn draft_text_edit_mode_seeds_the_input_and_hides_reset_when_unchanged() {
    let html = render(draft_text_edit_unmodified);
    assert!(html.contains("<input"), "edit mode swaps in an input:\n{html}");
    assert!(
        html.contains(r#"value="John""#),
        "the input is seeded from the record:\n{html}"
    );
    assert!(
        !html.contains("modified"),
        "an unchanged field is not modified:\n{html}"
    );
    assert!(
        !html.contains('↺'),
        "no reset control while the field is unchanged:\n{html}"
    );
}

#[test]
fn a_modified_draft_text_is_tinted_and_offers_a_labelled_reset() {
    let html = render(draft_text_edit_modified);
    assert!(
        html.contains(r#"class="in modified""#),
        "a changed field carries the `modified` class:\n{html}"
    );
    assert!(
        html.contains("Reset Given name to original value"),
        "the reset control's accessible name embeds the field label:\n{html}"
    );
}

// ---- DraftSelect ---------------------------------------------------------------------------------

fn draft_select_view() -> Element {
    rsx! {
        DraftSelect {
            label: "Sex".to_owned(),
            name: "sex".to_owned(),
            editing: false,
            value: "1".to_owned(),
            original: "1".to_owned(),
            reset_label: "Reset Sex to original value".to_owned(),
            options: vec![
                SelectChoice { value: "0".to_owned(), label: "Female".to_owned() },
                SelectChoice { value: "1".to_owned(), label: "Male".to_owned() },
            ],
            onchange: move |_: String| {},
            onreset: move |()| {},
        }
    }
}

#[test]
fn draft_select_view_mode_shows_the_selected_option_label() {
    let html = render(draft_select_view);
    assert!(html.contains(r#"class="val""#), "view mode renders a read box:\n{html}");
    assert!(html.contains("Male"), "the selected option's label is shown:\n{html}");
    assert!(!html.contains("<select"), "view mode has no live select:\n{html}");
}

// ---- Head actions + provenance -------------------------------------------------------------------

/// A view-mode edit state (Edit shown), and an edit state that is dirty-and-valid vs one that is
/// pristine, for the head-actions / provenance assertions.
fn head_actions_view_mode() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let edit = RecordEditState::<TagDraft> {
        editing: use_signal(|| false),
        seed: use_signal(TagDraft::new),
        draft: use_signal(TagDraft::new),
        prov: use_signal(ProvenanceDraft::default),
    };
    rsx! {
        {record_head_actions(&labels, edit, rsx! {}, use_callback(|_: (TagDraft, ProvenanceDraft)| {}))}
    }
}

fn dirty_edit() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let edit = RecordEditState::<TagDraft> {
        editing: use_signal(|| true),
        seed: use_signal(TagDraft::new),
        draft: use_signal(|| TagDraft {
            name: "Ancestor".to_owned(),
            ..TagDraft::new()
        }),
        prov: use_signal(ProvenanceDraft::default),
    };
    rsx! {
        {record_head_actions(&labels, edit, rsx! {}, use_callback(|_: (TagDraft, ProvenanceDraft)| {}))}
        {record_edit_provenance(&loc, edit)}
    }
}

fn clean_edit() -> Element {
    let loc = loc();
    let labels = RecordActionLabels::resolve(&loc);
    let edit = RecordEditState::<TagDraft> {
        editing: use_signal(|| true),
        seed: use_signal(TagDraft::new),
        draft: use_signal(TagDraft::new),
        prov: use_signal(ProvenanceDraft::default),
    };
    rsx! {
        {record_head_actions(&labels, edit, rsx! {}, use_callback(|_: (TagDraft, ProvenanceDraft)| {}))}
        {record_edit_provenance(&loc, edit)}
    }
}

#[test]
fn view_mode_head_actions_show_only_edit() {
    let html = render(head_actions_view_mode);
    assert!(html.contains(">Edit<"), "view mode offers Edit:\n{html}");
    assert!(!html.contains(">Save<"), "no Save in view mode:\n{html}");
    assert!(!html.contains(">Cancel<"), "no Cancel in view mode:\n{html}");
}

#[test]
fn a_dirty_edit_enables_save_and_shows_the_provenance_block() {
    let html = render(dirty_edit);
    assert!(html.contains(">Cancel<"), "edit mode offers Cancel:\n{html}");
    assert!(html.contains(">Save<"), "edit mode offers Save:\n{html}");
    assert!(
        !html.contains("disabled"),
        "Save is enabled when the draft is dirty and valid:\n{html}"
    );
    assert!(
        html.contains(r#"role="group""#),
        "the provenance block is present while dirty:\n{html}"
    );
    assert!(
        html.contains(r#"class="card""#),
        "the provenance block is a card (record-editing.html §5b):\n{html}"
    );
}

#[test]
fn a_clean_edit_disables_save_and_hides_the_provenance_block() {
    let html = render(clean_edit);
    assert!(html.contains(">Save<"), "edit mode offers Save:\n{html}");
    assert!(
        html.contains("disabled"),
        "Save is disabled with nothing to save:\n{html}"
    );
    assert!(
        !html.contains(r#"role="group""#),
        "no provenance block while the record is pristine:\n{html}"
    );
}

// ---- Create frame --------------------------------------------------------------------------------

fn create_frame() -> Element {
    let loc = loc();
    let actions = rsx! {
        button { class: "btn sm", "{loc.action_label(\"cancel\")}" }
        button { class: "btn sm primary", "{loc.action_label(\"save\")}" }
    };
    rsx! {
        {create_record_header(&loc.person_new_title(), &loc.record_draft_badge(), actions)}
    }
}

fn create_frame_with_body() -> Element {
    let loc = loc();
    let body = rsx! {
        p { class: "probe", "body content" }
    };
    create_record_frame(&loc.person_new_title(), &loc.record_draft_badge(), rsx! {}, body)
}

#[test]
fn the_create_frame_wraps_its_body_in_the_edit_mode_tab_body() {
    let html = render(create_frame_with_body);
    let head = html.find(r#"class="detail-head""#);
    let tab_body = html.find(r#"class="tab-body""#);
    let probe = html.find(r#"class="probe""#);
    assert!(
        head.is_some_and(|head| tab_body.is_some_and(|tab| head < tab))
            && tab_body.is_some_and(|tab| probe.is_some_and(|probe| tab < probe)),
        "the body renders inside a tab-body after the header, sharing edit mode's inset:\n{html}"
    );
}

#[test]
fn the_create_frame_titles_the_draft_and_carries_actions_in_the_head() {
    let html = render(create_frame);
    assert!(
        html.contains(r#"class="detail-head""#),
        "the create header is a detail-head:\n{html}"
    );
    assert!(
        html.contains("New person"),
        "the create title names the new record:\n{html}"
    );
    assert!(html.contains("draft · not saved"), "the draft badge is shown:\n{html}");
    assert!(
        html.contains(r#"class="head-actions""#),
        "Cancel/Save sit in the head-actions:\n{html}"
    );
    assert!(
        html.contains(">Save<") && html.contains(">Cancel<"),
        "both actions render:\n{html}"
    );
}
