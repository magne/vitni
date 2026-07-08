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
        Table { headers: vec!["Name".to_owned(), "Role".to_owned()],
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
            Modal { title: "Hidden".to_owned(), open: false, footer: rsx! {},
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
