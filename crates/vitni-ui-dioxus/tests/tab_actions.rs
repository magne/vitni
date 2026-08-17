//! SSR assertions for [`tab_frame`], the single fn in the crate that emits a collection tab's
//! `.tab-actions` bar (issue #314 slice 2): one shared bar over 46 previously independent
//! re-implementations, so a tab's action label is always the category's own [`ActionLabel`], never a
//! generic "Save".

use dioxus::prelude::*;
use vitni_ui::{ActionLabel, DetailTab, Localizer};
use vitni_ui_dioxus::screens::{TabActionTarget, tab_frame};

/// The stand-in side-panel form the test's `TabActionTarget::Form` arms — the tests never open it,
/// they only assert the bar that would arm it.
#[derive(Clone, PartialEq)]
struct TestForm;

fn loc() -> Localizer {
    Localizer::with_languages(None, &["en".parse().unwrap_or_default()])
}

/// Renders [`tab_frame`] over one [`ActionLabel`], the way a real screen's tab dispatcher would.
fn render_bar(action: ActionLabel) -> String {
    #[component]
    fn Harness(action: ActionLabel) -> Element {
        let loc = loc();
        let editing = use_signal(|| None::<TestForm>);
        let tab = DetailTab {
            id: "test",
            label: String::new(),
            count: None,
            action: Some(action),
        };
        tab_frame(
            &loc,
            &tab,
            TabActionTarget::Form(editing, TestForm),
            None,
            rsx! { div { "BODY" } },
        )
    }
    let mut vdom = VirtualDom::new_with_props(Harness, HarnessProps { action });
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
