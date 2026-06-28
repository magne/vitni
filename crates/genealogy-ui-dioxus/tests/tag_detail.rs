//! SSR assertions for the Tag detail (Phase 5 PR11): render the overview (name/priority/colour) and
//! the usage tab (records grouped by object type with counts + examples). Asserts the name, priority,
//! colour swatch, and the usage groups — and that the tag's aggregate id is never rendered (§9).

use dioxus::prelude::*;
use genealogy_app::UsingKind;
use genealogy_ui::{Localizer, TagDetail, TagUsageGroupVm, UsingRecordVm};
use genealogy_ui_dioxus::screens::{TagEditForm, tag_overview, tag_usage_tab};

/// A representative tag: "Direct ancestor", priority 1, red, carried by people and a family.
fn sample() -> TagDetail {
    TagDetail {
        id: "0190-secret-tag-id".to_owned(),
        title: "Direct ancestor".to_owned(),
        name: Some("Direct ancestor".to_owned()),
        color: Some("#e5534b".to_owned()),
        priority: Some(1),
        total: 3,
        usage: vec![
            TagUsageGroupVm {
                kind_label: "Person".to_owned(),
                count: 2,
                examples: vec![
                    UsingRecordVm {
                        kind: UsingKind::Person,
                        human_id: "I0042".to_owned(),
                        id: "0190-person-42".to_owned(),
                        label: "John Smith".to_owned(),
                        kind_label: "Person".to_owned(),
                    },
                    UsingRecordVm {
                        kind: UsingKind::Person,
                        human_id: "I0043".to_owned(),
                        id: "0190-person-43".to_owned(),
                        label: "Mary Doe".to_owned(),
                        kind_label: "Person".to_owned(),
                    },
                ],
            },
            TagUsageGroupVm {
                kind_label: "Family".to_owned(),
                count: 1,
                examples: vec![UsingRecordVm {
                    kind: UsingKind::Family,
                    human_id: "F0017".to_owned(),
                    id: "0190-family-17".to_owned(),
                    label: "Doe — Smith".to_owned(),
                    kind_label: "Family".to_owned(),
                }],
            },
        ],
        history: Vec::new(),
    }
}

/// Renders the overview and usage tabs together.
fn tag_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let editing = use_signal(|| None::<TagEditForm>);
    let detail = sample();
    rsx! {
        {tag_overview(&loc, &detail, editing)}
        {tag_usage_tab(&loc, &detail)}
    }
}

#[test]
fn overview_shows_name_priority_and_colour() {
    let mut vdom = VirtualDom::new(tag_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in ["Direct ancestor", "#e5534b"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn usage_groups_records_by_type_with_counts_and_examples() {
    let mut vdom = VirtualDom::new(tag_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in ["Person", "Family", "John Smith", "Mary Doe", "Doe — Smith"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn the_tag_id_is_never_rendered() {
    let mut vdom = VirtualDom::new(tag_view);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
}
