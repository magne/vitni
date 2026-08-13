//! SSR assertions for the Tag detail (Phase 5 rework): the editable-record header (colour dot, name,
//! priority/count subtitle, colour + priority badges — never the UUID) and the Usage tab (records
//! grouped by object type, up to three examples rendered as links, then an ellipsis).

use dioxus::prelude::*;
use vitni_app::{TagRef, UsingKind};
use vitni_ui::{Category, Localizer, RestrictionKind, TagDetail, TagDraft, TagUsageGroupVm, UsingRecordVm};
use vitni_ui_dioxus::components::TabItem;
use vitni_ui_dioxus::master_detail::DetailContainer;
use vitni_ui_dioxus::screens::{
    RecordActionLabels, record_head_actions, tag_chips, tag_edit_colour_card, tag_edit_tag_card, tag_overview,
    tag_usage_tab, use_record_edit,
};
use vitni_ui_dioxus::shell::nav_state::NavState;

/// Renders the tag detail header the way `tag_detail` builds it: a `DetailContainer` with a
/// colour-dot avatar, the name title, the priority/count subtitle, and the colour + priority badges —
/// and never the tag's UUID (data-model §9).
fn tag_header(loc: &Localizer, detail: &TagDetail) -> Element {
    let priority = detail.priority.unwrap_or(1);
    let color = detail.color.clone().unwrap_or_default();
    let active = use_signal(|| 0_usize);
    rsx! {
        DetailContainer {
            title: detail.title.clone(),
            subtitle: loc.tag_header_subtitle(priority, detail.total),
            avatar_color: color.clone(),
            badges: vec![color, loc.tag_priority_badge(priority)],
            extras: rsx! {},
            actions: rsx! {},
            tabs: Vec::<TabItem>::new(),
            active,
        }
    }
}

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
        restrictions: Vec::new(),
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
        {tag_header(&loc, &detail)}
    }
}

/// The tag detail head-actions in view mode: a single primary Edit button (Save/Cancel replace it in
/// edit mode). Renders `record_head_actions` over a view-mode edit state.
fn head_actions_view() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    use_context_provider(NavState::new);
    let edit = use_record_edit::<TagDraft>(Category::Tags, "T0001", &TagDraft::from_detail(&sample()));
    let labels = RecordActionLabels::resolve(&loc);
    let on_save = use_callback(|_: (TagDraft, vitni_ui::ProvenanceDraft)| {});
    rsx! {
        {record_head_actions(&labels, edit, rsx! {}, on_save)}
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
        {tag_chips(&loc, &tags)}
    }
}

fn render(view: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(view);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn overview_view_mode() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let detail = sample();
    use_context_provider(NavState::new);
    let edit = use_record_edit::<TagDraft>(Category::Tags, "T0001", &TagDraft::from_detail(&detail));
    let name_touched = use_signal(|| false);
    let picker_open = use_signal(|| false);
    rsx! {
        {tag_overview(&loc, &detail, edit, name_touched, picker_open)}
    }
}

fn overview_view_mode_with_restrictions() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let mut detail = sample();
    detail.restrictions = vec![RestrictionKind::Confidential];
    use_context_provider(NavState::new);
    let edit = use_record_edit::<TagDraft>(Category::Tags, "T0001", &TagDraft::from_detail(&detail));
    let name_touched = use_signal(|| false);
    let picker_open = use_signal(|| false);
    rsx! {
        {tag_overview(&loc, &detail, edit, name_touched, picker_open)}
    }
}

fn overview_edit_mode() -> Element {
    // Edit mode swaps in the editable record cards (what `TagRecordEditor` renders behind the
    // `editing` signal); rendered directly here so the SSR test needs no `AppCtx`.
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let committed = TagDraft::from_detail(&sample());
    let draft = use_signal(|| committed.clone());
    let name_touched = use_signal(|| false);
    let picker_open = use_signal(|| false);
    rsx! {
        {tag_edit_tag_card(&loc, draft, &committed, name_touched, false)}
        {tag_edit_colour_card(&loc, draft, &committed, picker_open)}
    }
}

fn overview_edit_mode_with_restrictions() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let mut seed = sample();
    seed.restrictions = vec![RestrictionKind::Locked];
    let committed = TagDraft::from_detail(&seed);
    let draft = use_signal(|| committed.clone());
    let name_touched = use_signal(|| false);
    let picker_open = use_signal(|| false);
    rsx! {
        {tag_edit_tag_card(&loc, draft, &committed, name_touched, false)}
        {tag_edit_colour_card(&loc, draft, &committed, picker_open)}
    }
}

#[test]
fn overview_is_read_first_with_no_inputs() {
    let html = render(overview_view_mode);
    assert!(
        html.contains("Direct ancestor"),
        "the name is shown as read text:\n{html}"
    );
    assert!(!html.contains("<input"), "view mode shows no live inputs:\n{html}");
}

#[test]
fn the_record_edit_lives_in_the_header_actions() {
    let html = render(head_actions_view);
    assert!(
        html.contains(">Edit<"),
        "a single Edit button is present in the head-actions in view mode:\n{html}"
    );
    assert!(
        !html.contains(">Save<") && !html.contains(">Cancel<"),
        "Save/Cancel appear only in edit mode:\n{html}"
    );
}

#[test]
fn overview_edit_mode_swaps_in_the_inputs() {
    let html = render(overview_edit_mode);
    assert!(html.contains("<input"), "edit mode swaps in the record inputs:\n{html}");
    assert!(
        html.contains("Direct ancestor"),
        "the name input is seeded from the record:\n{html}"
    );
    assert!(
        !html.contains(">Edit<"),
        "the Edit button is gone in edit mode:\n{html}"
    );
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
            {tag_header(&loc, &detail)}
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

#[test]
fn view_mode_shows_the_tags_current_restrictions() {
    let html = render(overview_view_mode_with_restrictions);
    assert!(
        html.contains(r#"data-kind="confidential""#),
        "the restriction toggle set is rendered:\n{html}"
    );
    assert!(
        html.contains(r#"aria-pressed="true""#),
        "the confidential restriction shows pressed:\n{html}"
    );
}

#[test]
fn view_mode_with_no_restrictions_shows_the_toggle_set_unpressed() {
    let html = render(overview_view_mode);
    assert!(
        html.contains(r#"data-kind="confidential""#),
        "the restriction toggle set is rendered even when nothing is set:\n{html}"
    );
    assert!(
        !html.contains(r#"aria-pressed="true""#),
        "nothing is pressed when the tag carries no restriction:\n{html}"
    );
}

#[test]
fn edit_mode_restriction_toggles_are_seeded_from_the_draft() {
    let html = render(overview_edit_mode_with_restrictions);
    assert!(
        html.contains(r#"data-kind="locked""#),
        "the restriction toggle set is rendered in edit mode:\n{html}"
    );
    assert!(
        html.contains(r#"aria-pressed="true""#),
        "the locked restriction shows pressed:\n{html}"
    );
}
