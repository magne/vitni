//! SSR assertions for the Tag detail (Phase 5 rework): the editable-record header (colour dot, name,
//! priority/count subtitle, colour + priority badges — never the UUID) and the Usage tab (records
//! grouped by object type, up to three examples rendered as links, then an ellipsis).

use dioxus::prelude::*;
use vitni_app::{TagRef, UsingKind};
use vitni_ui::{Category, Localizer, RestrictionKind, TagDetail, TagDraft, TagUsageGroupVm, UsingRecordVm};
use vitni_ui_dioxus::components::{BadgeSpec, TabItem};
use vitni_ui_dioxus::master_detail::DetailContainer;
use vitni_ui_dioxus::screens::{
    RecordActionLabels, RecordEditState, record_edit_provenance, record_head_actions, tag_chips, tag_edit_colour_card,
    tag_edit_tag_card, tag_overview, tag_usage_tab, use_record_edit,
};
use vitni_ui_dioxus::shell::nav_state::NavState;

/// The tag page's label column (`docs/mockups/tag.html:93-112`), one value across the `.grid-2` pair
/// so the Tag card and the Colour card line their values up on the same column. Set by the Colour
/// card's Preview row, whose Norwegian label `FORHÅNDSVISNING` renders 122px.
const TAG_LABEL_WIDTH: u32 = 130;

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
            badges: vec![
                BadgeSpec::with_dot(color.clone(), color),
                BadgeSpec::text(loc.tag_priority_badge(priority)),
            ],
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

/// A pristine edit state over `detail`'s draft, in edit mode — what the detail pane hands the record
/// cards once Edit is pressed.
fn edit_state(detail: &TagDetail) -> RecordEditState<TagDraft> {
    let committed = TagDraft::from_detail(detail);
    RecordEditState {
        editing: use_signal(|| true),
        seed: use_signal({
            let committed = committed.clone();
            move || committed
        }),
        draft: use_signal(move || committed),
        prov: use_signal(vitni_ui::ProvenanceDraft::default),
    }
}

fn overview_edit_mode() -> Element {
    // Edit mode swaps in the editable record cards (what `TagRecordEditor` renders behind the
    // `editing` signal); rendered directly here so the SSR test needs no `AppCtx`.
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let edit = edit_state(&sample());
    let committed = edit.seed.read().clone();
    let name_touched = use_signal(|| false);
    let picker_open = use_signal(|| false);
    rsx! {
        {tag_edit_tag_card(&loc, edit, name_touched, false)}
        {tag_edit_colour_card(&loc, edit.draft, &committed, picker_open)}
    }
}

fn overview_edit_mode_with_restrictions() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let mut seed = sample();
    seed.restrictions = vec![RestrictionKind::Locked];
    let edit = edit_state(&seed);
    let committed = edit.seed.read().clone();
    let name_touched = use_signal(|| false);
    let picker_open = use_signal(|| false);
    rsx! {
        {tag_edit_tag_card(&loc, edit, name_touched, false)}
        {tag_edit_colour_card(&loc, edit.draft, &committed, picker_open)}
    }
}

/// A tag draft differing from its committed seed in nothing but its restriction set — savable, and
/// the provenance block asks why (issue #315).
fn tag_restriction_change_only() -> Element {
    let loc = Localizer::with_languages(None, &["en".parse().unwrap_or_default()]);
    let labels = RecordActionLabels::resolve(&loc);
    let seed = TagDraft::from_detail(&sample());
    let mut draft = seed.clone();
    draft.restrictions = vec![RestrictionKind::Privacy];
    let edit = RecordEditState {
        editing: use_signal(|| true),
        seed: use_signal(move || seed),
        draft: use_signal(move || draft),
        prov: use_signal(vitni_ui::ProvenanceDraft::default),
    };
    rsx! {
        {record_head_actions(&labels, edit, rsx! {}, use_callback(|_: (TagDraft, vitni_ui::ProvenanceDraft)| {}))}
        {record_edit_provenance(&loc, edit)}
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

/// The 8px colour dot the header badge carries (`docs/mockups/tag.html:64`).
const BADGE_DOT: &str =
    r#"<span class="dot" style="width:8px;height:8px;border-radius:var(--r-pill);background:#e5534b"></span>"#;

#[test]
fn the_header_colour_badge_carries_a_dot_of_the_tags_colour() {
    let html = render(header_view);
    assert!(
        html.contains(&format!(r#"<span class="badge">{BADGE_DOT}#e5534b</span>"#)),
        "the swatch is a dot *inside* the colour badge (tag.html:64):\n{html}"
    );
    assert!(
        html.contains(r#"<span class="badge">priority 1</span>"#),
        "a badge with no colour stays plain text:\n{html}"
    );
}

#[test]
fn read_rows_are_one_line_fact_rows_at_the_tag_label_width() {
    let html = render(overview_view_mode);
    for label in ["Name", "Priority", "Restrictions", "Swatch", "Preview"] {
        assert!(
            html.contains(&format!(
                r#"<span class="field-label" style="width:{TAG_LABEL_WIDTH}px;margin:0">{label}</span>"#
            )),
            "the {label} row is a {TAG_LABEL_WIDTH}px-label fact-row (tag.html:93-112):\n{html}"
        );
    }
    assert_eq!(
        html.matches(r#"<div class="fact-row">"#).count(),
        5,
        "five read rows, all one-line:\n{html}"
    );
    assert!(
        !html.contains(r#"<div class="field">"#),
        "no read row stacks its label above its value any more:\n{html}"
    );
}

#[test]
fn the_read_colour_card_shows_a_swatch_and_a_preview_chip() {
    let html = render(overview_view_mode);
    assert!(
        html.contains(
            r#"<span class="dot swatch-dot" style="width:28px;height:28px;border-radius:var(--r-md);background:#e5534b;flex:none"></span>"#
        ),
        "the read Colour card draws the 28px swatch (tag.html:109):\n{html}"
    );
    assert!(
        html.contains(r#"<span class="field val mono">#e5534b</span>"#),
        "beside the hex in the monospace face:\n{html}"
    );
    assert!(
        html.contains(r#"<span class="chip"><span class="dot" style="background:#e5534b"></span>Direct ancestor"#),
        "and a preview chip carrying the colour dot (tag.html:112):\n{html}"
    );
}

#[test]
fn edit_mode_keeps_the_two_cards_and_puts_every_row_on_one_line() {
    let html = render(overview_edit_mode);
    assert_eq!(
        html.matches(r#"<div class="card">"#).count(),
        2,
        "the Tag and Colour cards stay separate in edit mode:\n{html}"
    );
    assert!(
        html.contains(&format!(
            r#"<label for="tag-color" class="field-label" style="width:{TAG_LABEL_WIDTH}px;margin:0">Swatch</label>"#
        )),
        "the Swatch row labels the hex input on one line at the tag width:\n{html}"
    );
    assert!(
        html.contains(&format!(
            r#"<span class="field-label" style="width:{TAG_LABEL_WIDTH}px;margin:0">Preview</span>"#
        )),
        "and the preview chip sits in a fact-row of its own (tag.html:165):\n{html}"
    );
    assert!(
        !html.contains(r#"<div class="field">"#),
        "no edit row stacks its label above its control:\n{html}"
    );
}

#[test]
fn the_edit_swatch_row_keeps_the_picker_button_and_the_revert_control() {
    let html = render(overview_edit_mode);
    assert!(
        html.contains(r#"class="swatch-btn""#),
        "the swatch button still opens the colour picker:\n{html}"
    );
    assert!(
        html.contains(r#"class="field-with-revert" style="flex:1;max-width:160px""#),
        "the hex input keeps its bounded revert container (tag.html:160):\n{html}"
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
        "the restriction set is rendered:\n{html}"
    );
    assert!(
        html.contains(r#"aria-pressed="true""#),
        "the confidential restriction shows pressed:\n{html}"
    );
}

#[test]
fn view_mode_with_no_restrictions_shows_the_set_unpressed() {
    let html = render(overview_view_mode);
    assert!(
        html.contains(r#"data-kind="confidential""#),
        "the restriction set is rendered even when nothing is set:\n{html}"
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
    assert!(!html.contains("resn-static"), "edit mode offers live toggles:\n{html}");
}

#[test]
fn view_mode_restrictions_are_static_not_toggles() {
    let html = render(overview_view_mode_with_restrictions);
    assert_eq!(
        html.matches("resn-static").count(),
        3,
        "view mode states every kind, statically (issue #315):\n{html}"
    );
    assert!(
        !html.contains("<button"),
        "a restriction is changed in edit mode, never by pressing a read row:\n{html}"
    );
}

#[test]
fn a_restriction_change_alone_makes_the_tag_savable() {
    let html = render(tag_restriction_change_only);
    assert!(
        !html.contains("disabled"),
        "a restriction change alone enables Save:\n{html}"
    );
    assert!(
        html.contains(r#"id="prov-reason""#),
        "and asks for the reason like any other change (issue #315):\n{html}"
    );
}
