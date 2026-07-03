//! SSR assertions for the Tag detail (Phase 5 rework): the editable-record header (colour dot, name,
//! priority/count subtitle, colour + priority badges — never the UUID) and the Usage tab (records
//! grouped by object type, up to three examples rendered as links, then an ellipsis).

use dioxus::prelude::*;
use genealogy_app::{TagRef, UsingKind};
use genealogy_ui::{Localizer, TagDetail, TagUsageGroupVm, UsingRecordVm};
use genealogy_ui_dioxus::screens::{tag_record_header, tag_usage_tab, tags_panel};
use genealogy_ui_dioxus::shell::nav_state::NavState;

/// A representative tag: "Direct ancestor", priority 1, red, carried by many people and one family.
fn sample() -> TagDetail {
    TagDetail {
        id: "0190-secret-tag-id".to_owned(),
        title: "Direct ancestor".to_owned(),
        name: Some("Direct ancestor".to_owned()),
        color: Some("#e5534b".to_owned()),
        priority: Some(1),
        total: 5,
        usage: vec![
            TagUsageGroupVm {
                kind_label: "Person".to_owned(),
                count: 4,
                examples: vec![
                    person("I0042", "John Smith"),
                    person("I0043", "Mary Doe"),
                    person("I0044", "Jonathan Smith"),
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

fn person(human_id: &str, name: &str) -> UsingRecordVm {
    UsingRecordVm {
        kind: UsingKind::Person,
        human_id: human_id.to_owned(),
        id: format!("uuid-{human_id}"),
        label: name.to_owned(),
        kind_label: "Person".to_owned(),
    }
}

fn header_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let detail = sample();
    rsx! {
        {tag_record_header(&loc, &detail)}
    }
}

fn usage_view() -> Element {
    // RecordLink (the Usage examples) resolves NavState from context, so the harness must provide it.
    use_context_provider(NavState::new);
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let detail = sample();
    rsx! {
        {tag_usage_tab(&loc, &detail)}
    }
}

fn applied_tags_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let tags = vec![TagRef {
        id: "0190-secret-tag-id".to_owned(),
        name: "Direct ancestor".to_owned(),
        color: Some("#e5534b".to_owned()),
        priority: Some(1),
    }];
    rsx! {
        {tags_panel(&loc, &tags)}
    }
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn header_shows_name_colour_and_priority_never_the_uuid() {
    let html = render(header_view);
    assert!(html.contains("Direct ancestor"), "the name is the title:\n{html}");
    assert!(html.contains("#e5534b"), "the colour badge shows the hex:\n{html}");
    assert!(
        html.contains("background:#e5534b"),
        "the avatar is a colour dot:\n{html}"
    );
    assert!(
        !html.contains("0190-secret-tag-id"),
        "the tag's aggregate id must never be rendered:\n{html}"
    );
    // The last chip is priority-only; the object count belongs to the subtitle, not the badge.
    assert!(
        html.contains("priority 1"),
        "priority badge shows the priority:\n{html}"
    );
    assert!(
        html.contains("applied to 5 objects"),
        "subtitle carries the count:\n{html}"
    );
}

#[test]
fn header_subtitle_uses_the_singular_for_one_object() {
    fn one_object_view() -> Element {
        let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
        let mut detail = sample();
        detail.total = 1;
        rsx! {
            {tag_record_header(&loc, &detail)}
        }
    }
    let html = render(one_object_view);
    assert!(
        html.contains("applied to 1 object"),
        "singular form for one object:\n{html}"
    );
    assert!(!html.contains("1 objects"), "no plural for a single object:\n{html}");
}

#[test]
fn usage_groups_records_by_type_with_links_and_an_ellipsis() {
    let html = render(usage_view);
    for needle in [
        "Person",
        "Family",
        "John Smith",
        "Mary Doe",
        "Jonathan Smith",
        "Doe — Smith",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
    // The Person group has 4 records but shows only three examples, so an ellipsis follows.
    assert!(
        html.contains('…'),
        "a group with more than three examples ends with an ellipsis:\n{html}"
    );
}

#[test]
fn usage_examples_are_clickable_record_links() {
    let html = render(usage_view);
    // RecordLink renders a `button.src-link`; the examples are navigable, not plain text.
    assert!(
        html.contains("src-link"),
        "usage examples render as record links:\n{html}"
    );
}

#[test]
fn an_applied_tag_chip_shows_name_and_colour_not_the_uuid() {
    let html = render(applied_tags_view);
    assert!(html.contains("Direct ancestor"), "the chip shows the tag name:\n{html}");
    assert!(
        html.contains("background:#e5534b"),
        "the chip carries a colour dot:\n{html}"
    );
    assert!(
        !html.contains("0190-secret-tag-id"),
        "an applied-tag chip must never render the tag's UUID:\n{html}"
    );
}
