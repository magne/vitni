//! SSR assertions for the shared record-form pieces (Phase 5 PR27): the no-reflow `DraftText` /
//! `DraftSelect` fields, the sticky-header `record_head_actions`, the dirty-gated
//! `record_edit_provenance`, the create-frame header, and the shared restriction display/field
//! (issue #315). Pure render-and-inspect — no window, no workspace — over `TagDraft` as a
//! representative record draft, and `SourceDraft` where a create-mode draft must hide a field.
//!
//! The row *shape* is asserted here too (issue #310): a record row is one `.fact-row` with the label
//! in a fixed-width column beside the value in **both** modes, and a `multiline` field is the one
//! exception that stays stacked.

use dioxus::prelude::*;
use vitni_ui::ActionLabel;
use vitni_ui::{Localizer, ProvenanceDraft, RestrictionKind, SourceDraft, TagDraft};
use vitni_ui_dioxus::components::{DraftSelect, DraftText, SelectChoice};
use vitni_ui_dioxus::screens::{
    RecordActionLabels, RecordEditState, create_record_frame, create_record_header, record_edit_provenance,
    record_head_actions, record_restrictions_field, restriction_display,
};

/// The source record page's own label column width (`docs/mockups/source.html:112-116`).
const SOURCE_LABEL_WIDTH: u32 = 110;

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

/// The `.field-label` a row at `width` must carry, inline style and all.
fn label_column(width: u32) -> String {
    format!(r#"class="field-label" style="width:{width}px;margin:0""#)
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
    assert!(
        html.contains(r#"class="field val""#),
        "view mode renders a read box:\n{html}"
    );
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

// ---- Row shape (issue #310) ----------------------------------------------------------------------

/// The same field at the source page's own label width, in each mode, plus the invalid/hinted and
/// `multiline` variants — the four shapes the row assertions below measure.
fn row_view() -> Element {
    rsx! {
        DraftText {
            label: "Title".to_owned(),
            name: "source-title".to_owned(),
            label_width: SOURCE_LABEL_WIDTH,
            editing: false,
            value: "Parish register".to_owned(),
            original: "Parish register".to_owned(),
            reset_label: "Reset Title".to_owned(),
            oninput: move |_: String| {},
            onreset: move |()| {},
        }
    }
}

fn row_edit() -> Element {
    rsx! {
        DraftText {
            label: "Title".to_owned(),
            name: "source-title".to_owned(),
            label_width: SOURCE_LABEL_WIDTH,
            editing: true,
            value: "Parish register".to_owned(),
            original: "Parish register".to_owned(),
            reset_label: "Reset Title".to_owned(),
            oninput: move |_: String| {},
            onreset: move |()| {},
        }
    }
}

fn row_edit_with_error_and_hint() -> Element {
    rsx! {
        DraftText {
            label: "Id".to_owned(),
            name: "source-id".to_owned(),
            label_width: SOURCE_LABEL_WIDTH,
            editing: true,
            value: String::new(),
            original: "S0001".to_owned(),
            reset_label: "Reset Id".to_owned(),
            error: Some("An id is required.".to_owned()),
            hint: Some("Leave empty to generate one.".to_owned()),
            oninput: move |_: String| {},
            onreset: move |()| {},
        }
    }
}

fn multiline_view() -> Element {
    rsx! {
        DraftText {
            label: "Content".to_owned(),
            name: "note-content".to_owned(),
            label_width: 90,
            editing: false,
            value: "# Heading".to_owned(),
            original: "# Heading".to_owned(),
            reset_label: "Reset Content".to_owned(),
            multiline: true,
            oninput: move |_: String| {},
            onreset: move |()| {},
        }
    }
}

fn multiline_edit() -> Element {
    rsx! {
        DraftText {
            label: "Content".to_owned(),
            name: "note-content".to_owned(),
            label_width: 90,
            editing: true,
            value: "# Heading".to_owned(),
            original: "# Heading".to_owned(),
            reset_label: "Reset Content".to_owned(),
            multiline: true,
            oninput: move |_: String| {},
            onreset: move |()| {},
        }
    }
}

fn select_row_edit() -> Element {
    rsx! {
        DraftSelect {
            label: "Type".to_owned(),
            name: "note-type".to_owned(),
            label_width: 90,
            editing: true,
            value: "1".to_owned(),
            original: "1".to_owned(),
            reset_label: "Reset Type".to_owned(),
            options: vec![
                SelectChoice { value: "0".to_owned(), label: "General".to_owned() },
                SelectChoice { value: "1".to_owned(), label: "Research".to_owned() },
            ],
            onchange: move |_: String| {},
            onreset: move |()| {},
        }
    }
}

#[test]
fn a_record_row_puts_its_label_beside_the_value_in_both_modes() {
    for (mode, view) in [
        ("view", row_view as fn() -> Element),
        ("edit", row_edit as fn() -> Element),
    ] {
        let html = render(view);
        assert!(
            html.contains(r#"class="fact-row""#),
            "{mode} mode is one .fact-row (record-editing.html:47-99):\n{html}"
        );
        assert!(
            html.contains(&label_column(SOURCE_LABEL_WIDTH)),
            "{mode} mode labels the row in the page's own {SOURCE_LABEL_WIDTH}px column:\n{html}"
        );
        assert!(
            !html.contains(r#"<div class="field">"#),
            "{mode} mode no longer stacks the label above the value:\n{html}"
        );
    }
}

#[test]
fn a_select_row_is_one_line_too() {
    let html = render(select_row_edit);
    assert!(
        html.contains(r#"class="fact-row""#),
        "a select row is a .fact-row:\n{html}"
    );
    assert!(
        html.contains(&label_column(90)),
        "and takes the page's label width:\n{html}"
    );
    assert!(html.contains("<select"), "the control is still a live select:\n{html}");
}

#[test]
fn a_multiline_row_stays_stacked_in_both_modes() {
    for (mode, view) in [
        ("view", multiline_view as fn() -> Element),
        ("edit", multiline_edit as fn() -> Element),
    ] {
        let html = render(view);
        assert!(
            html.contains(r#"<div class="field">"#),
            "{mode} mode keeps the label above a Markdown body (note.html:131-136):\n{html}"
        );
        assert!(!html.contains("fact-row"), "a stacked field is not a row:\n{html}");
    }
}

#[test]
fn a_rows_error_and_hint_sit_under_the_input_not_beside_it() {
    let html = render(row_edit_with_error_and_hint);
    let grow = html.find(r#"class="grow""#);
    let input = html.find("<input");
    let error = html.find(r#"class="field-error""#);
    let hint = html.find(r#"class="field-hint""#);
    assert!(
        grow.is_some_and(|grow| input.is_some_and(|input| grow < input)),
        "the control is wrapped in the row's .grow cell:\n{html}"
    );
    assert!(
        input.is_some_and(|input| error.is_some_and(|error| input < error))
            && error.is_some_and(|error| hint.is_some_and(|hint| error < hint)),
        "the message and hint follow the input inside that cell, so .fact-row's wrap cannot put them \
         beside it:\n{html}"
    );
    assert_eq!(
        html.matches("fact-row").count(),
        1,
        "and the row itself is still one row:\n{html}"
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
    assert!(
        html.contains(r#"class="field val""#),
        "view mode renders a read box:\n{html}"
    );
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

// ---- Restrictions --------------------------------------------------------------------------------

/// A stored source carrying `restrictions` — a draft whose `editable_restrictions` is `Some`.
fn stored_source(restrictions: Vec<RestrictionKind>) -> SourceDraft {
    SourceDraft {
        existing_human_id: Some("S0001".to_owned()),
        human_id: "S0001".to_owned(),
        title: "Parish register".to_owned(),
        restrictions,
        ..SourceDraft::new()
    }
}

/// The edit state for `draft`, seeded from itself (pristine) in `editing` mode.
fn source_record(editing: bool, draft: SourceDraft) -> RecordEditState<SourceDraft> {
    let seed = draft.clone();
    RecordEditState {
        editing: use_signal(move || editing),
        seed: use_signal(move || seed),
        draft: use_signal(move || draft),
        prov: use_signal(ProvenanceDraft::default),
    }
}

fn restriction_field_view_mode() -> Element {
    record_restrictions_field(
        &loc(),
        source_record(false, stored_source(vec![RestrictionKind::Privacy])),
        SOURCE_LABEL_WIDTH,
    )
}

fn restriction_field_edit_mode() -> Element {
    record_restrictions_field(
        &loc(),
        source_record(true, stored_source(vec![RestrictionKind::Privacy])),
        SOURCE_LABEL_WIDTH,
    )
}

fn restriction_field_create_mode() -> Element {
    record_restrictions_field(&loc(), source_record(true, SourceDraft::new()), SOURCE_LABEL_WIDTH)
}

#[test]
fn a_create_draft_renders_no_restriction_field() {
    let html = render(restriction_field_create_mode);
    assert!(
        !html.contains("resn"),
        "a draft with no stored record behind it offers no restriction field:\n{html}"
    );
}

#[test]
fn view_mode_restrictions_are_static_pills_over_every_kind() {
    let html = render(restriction_field_view_mode);
    assert!(html.contains(">Restrictions<"), "the field is labelled:\n{html}");
    assert!(!html.contains("<button"), "view mode offers nothing to press:\n{html}");
    for kind in ["confidential", "locked", "privacy"] {
        assert!(
            html.contains(&format!(r#"data-kind="{kind}""#)),
            "all three kinds render, so edit mode reflows nothing:\n{html}"
        );
    }
    assert_eq!(
        html.matches("resn-static").count(),
        3,
        "every pill is static in view mode:\n{html}"
    );
}

#[test]
fn edit_mode_restrictions_are_live_toggles_seeded_from_the_draft() {
    let html = render(restriction_field_edit_mode);
    assert!(html.contains("<button"), "edit mode offers live toggles:\n{html}");
    assert!(!html.contains("resn-static"), "no static pill in edit mode:\n{html}");
    assert_eq!(
        html.matches(r#"aria-pressed="true""#).count(),
        1,
        "only the draft's own restriction is pressed:\n{html}"
    );
    assert_eq!(
        html.matches(r#"aria-pressed="false""#).count(),
        2,
        "the other two read as off:\n{html}"
    );
}

fn restriction_display_unrestricted() -> Element {
    restriction_display(&loc(), &[])
}

fn restriction_display_privacy() -> Element {
    restriction_display(&loc(), &[RestrictionKind::Privacy])
}

#[test]
fn an_unrestricted_record_displays_no_restriction_chips() {
    let html = render(restriction_display_unrestricted);
    assert!(
        !html.contains("resn"),
        "nothing is in force, so the header shows nothing:\n{html}"
    );
}

#[test]
fn the_display_shows_only_the_restrictions_in_force() {
    let html = render(restriction_display_privacy);
    assert!(html.contains(r#"data-kind="privacy""#), "the set kind shows:\n{html}");
    assert!(
        !html.contains(r#"data-kind="locked""#) && !html.contains(r#"data-kind="confidential""#),
        "an unset kind is not shown at all:\n{html}"
    );
    assert!(html.contains("resn-static"), "the header's chips are static:\n{html}");
    assert!(!html.contains("<button"), "the header offers nothing to press:\n{html}");
    assert!(
        html.contains(r#"aria-label="Privacy restrictions""#),
        "the group is named:\n{html}"
    );
}

// ---- Create frame --------------------------------------------------------------------------------

fn create_frame() -> Element {
    let loc = loc();
    let actions = rsx! {
        button { class: "btn sm", "{loc.action_button(ActionLabel::Cancel)}" }
        button { class: "btn sm primary", "{loc.action_button(ActionLabel::Save)}" }
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
