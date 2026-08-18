//! SSR assertions for [`tab_frame`], the shared record-tab frame: the tab's explanation, then the
//! `.tab-actions` bar it is the only emitter of (issue #314 slice 2), then the tab's own body
//! (issue #303). One frame over 46 previously independent re-implementations, so a tab's action
//! label is always the category's own [`ActionLabel`] and its explanation always survives the empty
//! state.

use dioxus::prelude::*;
use vitni_app::TagRef;
use vitni_ui::{ActionLabel, DetailTab, HistoryEntryVm, Localizer};
use vitni_ui_dioxus::screens::{TabActionTarget, history_panel, tab_frame, tags_panel};

/// The stand-in side-panel form the test's `TabActionTarget::Form` arms — the tests never open it,
/// they only assert the bar that would arm it.
#[derive(Clone, PartialEq)]
struct TestForm;

/// The tabs every aggregate renders the same way, which the frame explains (issue #303).
const SHARED_TAB_IDS: &[&str] = &["citations", "media", "notes", "tags", "addresses", "history"];

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

/// Renders [`tab_frame`] over one [`ActionLabel`], the way a real screen's tab dispatcher would.
/// The tab id is `"test"` — no shared tab, so these assertions see the bar alone.
fn render_bar(action: ActionLabel) -> String {
    render_tab("test", Some(action), false)
}

/// Renders [`tab_frame`] the way a screen's tab dispatcher does, over an arbitrary tab id: `action`
/// `None` is a read-only tab (`TabActionTarget::None`), and `empty` swaps the populated body for the
/// empty state a collection tab renders when it holds nothing.
fn render_tab(id: &'static str, action: Option<ActionLabel>, empty: bool) -> String {
    #[component]
    fn Harness(id: &'static str, action: Option<ActionLabel>, empty: bool) -> Element {
        let loc = loc();
        let editing = use_signal(|| None::<TestForm>);
        let tab = DetailTab {
            id,
            label: String::new(),
            count: None,
            action,
        };
        let body = if empty {
            rsx! { div { class: "empty", "EMPTY" } }
        } else {
            rsx! { div { "BODY" } }
        };
        let target = match action {
            Some(_) => TabActionTarget::Form(editing, TestForm),
            None => TabActionTarget::None,
        };
        tab_frame(&loc, &tab, target, None, body)
    }
    let mut vdom = VirtualDom::new_with_props(Harness, HarnessProps { id, action, empty });
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn a_collection_tab_renders_one_action_bar_above_its_body() {
    let html = render_bar(ActionLabel::AddFact);
    let bar_count = html.matches(r#"class="tab-actions""#).count();
    assert_eq!(bar_count, 1, "exactly one action bar:\n{html}");
    let bar_at = html.find(r#"class="tab-actions""#).expect("the bar renders");
    let body_at = html.find("BODY").expect("the body renders");
    assert!(bar_at < body_at, "the bar precedes the body:\n{html}");
}

#[test]
fn the_action_bar_uses_the_categorys_own_label_never_save() {
    for (action, expected) in [
        (ActionLabel::AddParticipant, "Add participant"),
        (ActionLabel::AddAddress, "Add address"),
        (ActionLabel::AddUrl, "Add URL"),
        (ActionLabel::LinkSource, "Link source"),
        (ActionLabel::LinkRepository, "Link repository"),
        (ActionLabel::AddEnclosing, "Add enclosing place"),
    ] {
        let html = render_bar(action);
        assert!(
            html.contains(expected),
            "{action:?} should render {expected:?}:\n{html}"
        );
        assert!(
            !html.contains(">Save<"),
            "{action:?} must never fall back to Save:\n{html}"
        );
    }
}

fn read_only_tab() -> Element {
    let loc = loc();
    let tab = DetailTab {
        id: "test",
        label: String::new(),
        count: None,
        action: None,
    };
    tab_frame::<TestForm>(&loc, &tab, TabActionTarget::None, None, rsx! { div { "BODY" } })
}

#[test]
fn a_read_only_tab_renders_no_action_bar() {
    let html = render(read_only_tab);
    assert!(!html.contains("tab-actions"), "a read-only tab has no bar:\n{html}");
    assert!(html.contains("BODY"), "the body still renders:\n{html}");
}

#[test]
fn the_button_carries_the_affordance_glyph() {
    let html = render_bar(ActionLabel::AddSegment);
    assert!(
        html.contains("+ Add segment"),
        "the create affordance carries its + glyph:\n{html}"
    );
}

/// The frame's order is explanation → action bar → body: the button follows the sentence that says
/// what pressing it asserts, not the other way round (issue #303).
#[test]
fn a_shared_tabs_explanation_precedes_its_action_bar() {
    let html = render_tab("citations", Some(ActionLabel::AttachCitation), false);
    let note_at = html
        .find(r#"class="section-note""#)
        .expect("the shared tab renders its explanation");
    let bar_at = html.find(r#"class="tab-actions""#).expect("the bar renders");
    let body_at = html.find("BODY").expect("the body renders");
    assert!(note_at < bar_at, "the explanation precedes the bar:\n{html}");
    assert!(bar_at < body_at, "the bar precedes the body:\n{html}");
}

/// The explanation used to live inside each body, behind its `EmptyState` early return, so it
/// vanished exactly when a new operator had nothing else to infer the tab's meaning from.
#[test]
fn a_shared_tabs_explanation_survives_the_empty_state() {
    for id in SHARED_TAB_IDS {
        let html = render_tab(id, Some(ActionLabel::AttachNote), true);
        assert!(
            html.contains(r#"class="section-note""#),
            "{id} keeps its explanation when empty:\n{html}"
        );
        assert!(html.contains("EMPTY"), "{id} still renders its empty state:\n{html}");
    }
}

/// History is read-only — no bar — and was the one shared tab that already had an explanation, from
/// inside `history_panel`. The frame owns it now, so it must render without an action to hang it on.
#[test]
fn a_read_only_shared_tab_renders_its_explanation_without_a_bar() {
    let html = render_tab("history", None, false);
    assert!(
        html.contains(r#"class="section-note""#),
        "History explains itself through the frame:\n{html}"
    );
    assert_eq!(
        html.matches(r#"class="section-note""#).count(),
        1,
        "exactly one explanation — the frame is the only site:\n{html}"
    );
    assert!(!html.contains("tab-actions"), "a read-only tab has no bar:\n{html}");
    assert!(html.contains("BODY"), "the body still renders:\n{html}");
}

/// The frame explains the tabs every aggregate shares, and stays silent on the ones a screen owns:
/// a note on `names` or `facts` would be a claim about that one entity, not the shared vocabulary.
#[test]
fn a_screen_owned_tab_gets_no_explanation_from_the_frame() {
    for id in ["names", "facts", "participants", "segments"] {
        let html = render_tab(id, Some(ActionLabel::AddFact), false);
        assert!(
            !html.contains("section-note"),
            "{id} is the screen's own tab and the frame must not explain it:\n{html}"
        );
        assert!(html.contains("tab-actions"), "{id} still gets its bar:\n{html}");
    }
}

/// The real History tab, frame and body together: `history_panel` used to carry its own
/// `.section-note`, so once the frame emits one the two must not both fire.
#[test]
fn the_real_history_tab_renders_exactly_one_explanation() {
    fn history_tab() -> Element {
        let loc = loc();
        let tab = DetailTab {
            id: "history",
            label: String::new(),
            count: None,
            action: None,
        };
        let entries = vec![HistoryEntryVm {
            when: "2026-06-22 14:35".to_owned(),
            what: "Name asserted".to_owned(),
            who: "magne".to_owned(),
            why: None,
            assertion_id: "a-1".to_owned(),
            can_undo: true,
        }];
        tab_frame::<TestForm>(
            &loc,
            &tab,
            TabActionTarget::None,
            None,
            history_panel(&loc, &entries, None),
        )
    }

    let html = render(history_tab);
    assert_eq!(
        html.matches(r#"class="section-note""#).count(),
        1,
        "the frame is the only explanation site:\n{html}"
    );
    assert!(html.contains("Name asserted"), "the timeline still renders:\n{html}");
}

/// An empty History tab is the case the old placement lost: `history_panel` early-returns its empty
/// state, so a note inside it never reached a record with no changes yet.
#[test]
fn an_empty_history_tab_still_explains_itself() {
    fn empty_history_tab() -> Element {
        let loc = loc();
        let tab = DetailTab {
            id: "history",
            label: String::new(),
            count: None,
            action: None,
        };
        tab_frame::<TestForm>(&loc, &tab, TabActionTarget::None, None, history_panel(&loc, &[], None))
    }

    let html = render(empty_history_tab);
    assert!(
        html.contains(r#"class="section-note""#),
        "an empty History tab explains what it will hold:\n{html}"
    );
    assert!(
        html.contains("audit trail"),
        "and it is still the audit-trail wording:\n{html}"
    );
}

/// The Tags tab's chips read a size larger than a table cell's chips, which is a class on their
/// container rather than a change to `.chip` — the chips in every table stay as they are.
#[test]
fn the_tags_tab_marks_its_chips_as_the_larger_set() {
    fn tags_tab() -> Element {
        let loc = loc();
        let tags = vec![TagRef {
            id: "11111111-1111-7111-8111-111111111111".to_owned(),
            name: "Direct ancestor".to_owned(),
            color: Some("#e5534b".to_owned()),
            priority: None,
        }];
        tags_panel(&loc, &tags, Callback::new(|_: (String, String)| {}))
    }

    let html = render(tags_tab);
    assert!(
        html.contains(r#"class="wrap tag-chips""#),
        "the chip container carries `.tag-chips`:\n{html}"
    );
    assert!(
        html.contains("Direct ancestor"),
        "the tag still renders by name:\n{html}"
    );
}
