//! SSR assertions for the design-system components (Phase 5 PR1, ADR 0008 §5): render a gallery of
//! every component to an HTML string and assert each carries the ARIA roles/labels and the
//! colour-not-alone text the accessibility gate requires. No window, no plugin host — pure
//! render-and-inspect, the same pattern as `interpreter.rs`.

use dioxus::prelude::*;
use genealogy_ui::{ConfidenceLevel, EvidenceAxis, RestrictionKind};
use genealogy_ui_dioxus::components::{
    Badge, Breadcrumb, Button, ButtonVariant, Card, Checkbox, Chip, ConfidenceBadge, EmptyState, EvidenceAxisChip,
    HistoryEntry, HistoryTimeline, IconButton, Input, LabeledValue, ListRow, Modal, NoSourceFlag, NumberInput,
    ProvenancePopover, RestrictionChoice, RestrictionSet, Select, SelectChoice, SidePanel, SourceLink, StatusLine,
    TabItem, Table, Tabs, Toast,
};

/// A gallery instantiating each component with representative props.
fn gallery() -> Element {
    rsx! {
        Button { label: "Save".to_owned(), variant: ButtonVariant::Primary, onclick: move |_| {} }
        Button {
            label: "Retract".to_owned(),
            variant: ButtonVariant::Danger,
            title: Some("Retract this assertion — it stays in History".to_owned()),
            aria_label: Some("Retract Birth".to_owned()),
            onclick: move |_| {},
        }
        IconButton { icon: "✕".to_owned(), label: "Close".to_owned(), onclick: move |_| {} }
        Input { label: "Given name".to_owned(), name: "given".to_owned() }
        NumberInput { label: "Year".to_owned(), name: "year".to_owned() }
        Checkbox { label: "Private".to_owned(), name: "private".to_owned() }
        Select {
            label: "Sex".to_owned(),
            name: "sex".to_owned(),
            options: vec![
                SelectChoice { value: "male".to_owned(), label: "male".to_owned() },
                SelectChoice { value: "female".to_owned(), label: "female".to_owned() },
            ],
        }
        LabeledValue { label: "ID".to_owned(), value: "I0042".to_owned() }
        Tabs {
            tabs: vec![
                TabItem { id: "overview".to_owned(), label: "Overview".to_owned(), count: None },
                TabItem { id: "facts".to_owned(), label: "Facts".to_owned(), count: Some(3) },
            ],
            active: 0,
            onselect: move |_| {},
            div { "Overview pane" }
        }
        Table { caption: "Participants".to_owned(), headers: vec!["Name".to_owned(), "Role".to_owned()],
            tr { td { "Smith, John" } td { "ancestor" } }
        }
        ListRow {
            title: "Smith, John".to_owned(),
            subtitle: "male".to_owned(),
            id_label: "I0042".to_owned(),
            selected: true,
            onclick: move |_| {},
        }
        Badge { label: "I0042".to_owned() }
        Chip { label: "birth name".to_owned() }
        ConfidenceBadge { level: ConfidenceLevel::High, label: "High".to_owned() }
        EvidenceAxisChip { axis: EvidenceAxis::Source, label: "original".to_owned() }
        NoSourceFlag { label: "no source".to_owned() }
        SourceLink { label: "2 sources".to_owned(), onclick: move |_| {} }
        RestrictionSet {
            choices: vec![
                RestrictionChoice { kind: RestrictionKind::Confidential, label: "Confidential".to_owned() },
                RestrictionChoice { kind: RestrictionKind::Locked, label: "Locked".to_owned() },
            ],
            selected: vec![RestrictionKind::Confidential],
            ontoggle: move |_| {},
        }
        ProvenancePopover { title: "Why we believe this".to_owned(),
            div { "Baptism register" }
        }
        HistoryTimeline {
            entries: vec![HistoryEntry {
                when: "2026-06-22".to_owned(),
                what: "Birth asserted".to_owned(),
                who: "magne".to_owned(),
                why: Some("Baptism register".to_owned()),
                assertion_id: "a1".to_owned(),
                can_undo: true,
                undo_text: "Undo".to_owned(),
                undo_label: "Undo: Birth asserted".to_owned(),
            }],
            onundo: move |_| {},
        }
        Toast { visible: true, message: "Saved".to_owned(), action_label: Some("Undo".to_owned()), onaction: move |_| {} }
        Card { title: Some("Facts".to_owned()),
            div { "card body" }
        }
        EmptyState { message: "No citations yet.".to_owned() }
        Breadcrumb { segments: vec!["Genealogy".to_owned(), "People".to_owned()] }
        StatusLine { active_record: "Smith, John".to_owned(),
            span { "1 of 42" }
        }
        SidePanel {
            title: "Edit birth fact".to_owned(),
            open: true,
            close_label: "Close".to_owned(),
            onclose: move |_| {},
            footer: rsx! { Button { label: "Save".to_owned(), onclick: move |_| {} } },
            div { "panel body" }
        }
        Modal {
            title: "Delete tag?".to_owned(),
            open: true,
            close_label: "Dismiss".to_owned(),
            onclose: move |()| {},
            footer: rsx! { Button { label: "Delete".to_owned(), variant: ButtonVariant::Danger, onclick: move |_| {} } },
            div { "This removes the tag." }
        }
    }
}

fn render() -> String {
    let mut vdom = VirtualDom::new(gallery);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn components_carry_their_aria_roles() {
    let html = render();
    for needle in [
        r#"role="tablist""#,
        r#"role="tab""#,
        r#"role="tabpanel""#,
        r#"role="option""#,
        r#"role="dialog""#,
        r#"role="contentinfo""#,
        r#"role="status""#,
        r#"aria-selected="true""#,
        r#"aria-modal="true""#,
        r#"aria-pressed="true""#,
        r#"aria-pressed="false""#,
        r#"aria-live="polite""#,
        r#"aria-controls="panel-overview""#,
        r#"aria-label="Close""#,
        r#"aria-label="Edit birth fact""#,
        // A labelled Button can carry a hover tooltip and an accessible name (PR29 row actions).
        r#"aria-label="Retract Birth""#,
        r#"title="Retract this assertion — it stays in History""#,
        // Tabs use roving tabindex: the active tab is the stop, the rest are not.
        r#"tabindex="0""#,
        r#"tabindex="-1""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in rendered HTML:\n{html}");
    }
}

#[test]
fn semantic_cues_render_colour_not_alone() {
    let html = render();
    // Confidence: the data-level token AND a redundant text label.
    assert!(html.contains(r#"data-level="high""#), "confidence data-level:\n{html}");
    assert!(html.contains(">High"), "confidence text label:\n{html}");
    // Evidence axis: the hue class AND the axis text.
    assert!(html.contains("ev source"), "evidence axis class:\n{html}");
    assert!(html.contains("original"), "evidence axis text:\n{html}");
    // No-source: an icon plus text, not colour alone.
    assert!(html.contains("no source"), "no-source text:\n{html}");
    // Restriction set keys on data-kind, not colour.
    assert!(
        html.contains(r#"data-kind="confidential""#),
        "restriction data-kind:\n{html}"
    );
}

#[test]
fn form_inputs_render_native_controls() {
    let html = render();
    assert!(html.contains("<select"), "select renders a <select>:\n{html}");
    assert!(html.contains(r#"type="checkbox""#), "checkbox renders:\n{html}");
    assert!(html.contains(r#"type="number""#), "number input renders:\n{html}");
}

#[test]
fn controlled_overlays_hide_when_closed() {
    fn closed() -> Element {
        rsx! {
            Modal {
                title: "Hidden".to_owned(),
                open: false,
                close_label: "Dismiss".to_owned(),
                onclose: move |()| {},
                footer: rsx! {},
                div { "body" }
            }
            Toast { visible: false, message: "nope".to_owned() }
        }
    }
    let mut vdom = VirtualDom::new(closed);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);
    assert!(!html.contains("Hidden"), "closed modal renders nothing:\n{html}");
    assert!(!html.contains("nope"), "hidden toast renders nothing:\n{html}");
}

/// A range date prefilled for editing, exercising the structured [`DraftDate`] cluster.
fn draft_date_range_edit() -> Element {
    use genealogy_ui::{DateDraft, DateModifierKind};
    use genealogy_ui_dioxus::components::DraftDate;

    let value = DateDraft {
        kind: DateModifierKind::Range,
        start: "1876".to_owned(),
        end: "1880".to_owned(),
        original_text: "between 1876 and 1880".to_owned(),
        display: "between 1876 and 1880".to_owned(),
        ..DateDraft::default()
    };
    rsx! {
        DraftDate {
            label: "Date".to_owned(),
            name: "event-date".to_owned(),
            editing: true,
            value: value.clone(),
            original: DateDraft::default(),
            modifier_options: vec![
                SelectChoice { value: "0".to_owned(), label: "exact".to_owned() },
                SelectChoice { value: "4".to_owned(), label: "range".to_owned() },
            ],
            quality_options: vec![SelectChoice { value: "0".to_owned(), label: "normal".to_owned() }],
            calendar_options: vec![SelectChoice { value: "0".to_owned(), label: "Gregorian".to_owned() }],
            modifier_label: "Date modifier".to_owned(),
            date_label: "Date".to_owned(),
            quality_label: "Date quality".to_owned(),
            calendar_label: "Calendar".to_owned(),
            end_label: "End date".to_owned(),
            original_label: "Original text".to_owned(),
            original_hint: "The verbatim source string — always retained.".to_owned(),
            reset_label: "Reset Date".to_owned(),
            onchange: move |_: DateDraft| {},
            onreset: move |()| {},
        }
    }
}

/// An invalid date, exercising the error/`aria-invalid` path.
fn draft_date_invalid() -> Element {
    use genealogy_ui::DateDraft;
    use genealogy_ui_dioxus::components::DraftDate;

    let value = DateDraft {
        start: "gibberish".to_owned(),
        ..DateDraft::default()
    };
    rsx! {
        DraftDate {
            label: "Date".to_owned(),
            name: "event-date".to_owned(),
            editing: true,
            value: value.clone(),
            original: DateDraft::default(),
            modifier_options: vec![SelectChoice { value: "0".to_owned(), label: "exact".to_owned() }],
            quality_options: vec![SelectChoice { value: "0".to_owned(), label: "normal".to_owned() }],
            calendar_options: vec![SelectChoice { value: "0".to_owned(), label: "Gregorian".to_owned() }],
            modifier_label: "Date modifier".to_owned(),
            date_label: "Date".to_owned(),
            quality_label: "Date quality".to_owned(),
            calendar_label: "Calendar".to_owned(),
            end_label: "End date".to_owned(),
            original_label: "Original text".to_owned(),
            original_hint: "hint".to_owned(),
            reset_label: "Reset Date".to_owned(),
            error: Some("Not a valid date.".to_owned()),
            onchange: move |_: DateDraft| {},
            onreset: move |()| {},
        }
    }
}

/// A read-first date in view mode: the localized read box, no controls.
fn draft_date_view() -> Element {
    use genealogy_ui::DateDraft;
    use genealogy_ui_dioxus::components::DraftDate;

    let value = DateDraft {
        start: "1876".to_owned(),
        display: "1876".to_owned(),
        ..DateDraft::default()
    };
    rsx! {
        DraftDate {
            label: "Date".to_owned(),
            name: "event-date".to_owned(),
            editing: false,
            value: value.clone(),
            original: value,
            modifier_options: Vec::new(),
            quality_options: Vec::new(),
            calendar_options: Vec::new(),
            modifier_label: "Date modifier".to_owned(),
            date_label: "Date".to_owned(),
            quality_label: "Date quality".to_owned(),
            calendar_label: "Calendar".to_owned(),
            end_label: "End date".to_owned(),
            original_label: "Original text".to_owned(),
            original_hint: "hint".to_owned(),
            reset_label: "Reset Date".to_owned(),
            onchange: move |_: DateDraft| {},
            onreset: move |()| {},
        }
    }
}

fn render_view(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn draft_date_edit_mode_carries_the_control_cluster() {
    let html = render_view(draft_date_range_edit);
    for needle in [
        r#"aria-label="Date modifier""#,
        r#"aria-label="Date""#,
        r#"aria-label="Date quality""#,
        r#"aria-label="Calendar""#,
        r#"aria-label="End date""#,
        r#"aria-label="Original text""#,
        r#"value="1876""#,
        r#"value="1880""#,
        "The verbatim source string",
        r#"aria-label="Reset Date""#,
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn draft_date_invalid_marks_the_input_and_shows_the_error() {
    let html = render_view(draft_date_invalid);
    assert!(
        html.contains(r#"aria-invalid="true""#),
        "the start input is marked invalid:\n{html}"
    );
    assert!(
        html.contains("Not a valid date."),
        "the localized error renders:\n{html}"
    );
}

#[test]
fn draft_date_view_mode_is_a_read_box_without_controls() {
    let html = render_view(draft_date_view);
    assert!(html.contains(r#"class="val""#), "view mode shows a read box:\n{html}");
    assert!(!html.contains("<select"), "view mode shows no selects:\n{html}");
    assert!(html.contains("1876"), "the display string shows:\n{html}");
}

fn deletable_chip() -> Element {
    rsx! {
        Chip {
            label: "family".to_owned(),
            dot_color: "#3b82f6".to_owned(),
            id_label: "T0007".to_owned(),
            delete_label: "Remove tag family".to_owned(),
            delete_title: "Untag".to_owned(),
            ondelete: move |()| {},
        }
    }
}

#[test]
fn deletable_chip_carries_the_delete_control_inside_the_chip() {
    let html = render_view(deletable_chip);
    assert!(html.contains(r#"class="chip""#), "renders a chip:\n{html}");
    assert!(
        html.contains(r#"class="chip-delete""#),
        "the delete control sits inside the chip:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Remove tag family""#),
        "the delete control has its accessible name:\n{html}"
    );
    assert!(html.contains("T0007"), "the trailing id renders:\n{html}");
}
