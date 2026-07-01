//! SSR assertions for the Pedigree tool (Phase 5 PR 18): the ancestor/descendant charts render as
//! an accessible `role="tree"` of `role="treeitem"` nodes (never a dead end — an unresearched
//! ancestor slot is its own placeholder treeitem), and the view switcher's labels are localized.
//! Pure render-and-inspect over hand-built view-models — no window, no workspace — the same pattern
//! as `history_dashboard.rs`/`person_detail.rs`.

use std::rc::Rc;

use dioxus::prelude::*;
use genealogy_ui::{ConfidenceLevel, PedigreeNodeVm, PedigreeSlotVm};
use genealogy_ui_dioxus::components::{TabItem, Tabs};
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::screens::{AncestorTreeView, DescendantTreeView};
use genealogy_ui_dioxus::shell::ChromeCtx;
use genealogy_ui_dioxus::shell::nav_state::NavState;
use unic_langid::LanguageIdentifier;

fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

fn node(human_id: &str, name: &str, confidence: ConfidenceLevel) -> PedigreeNodeVm {
    PedigreeNodeVm {
        human_id: human_id.to_owned(),
        name: name.to_owned(),
        vitals: Some("1850 – 1920".to_owned()),
        confidence: Some(confidence),
        confidence_label: Some(
            match confidence {
                ConfidenceLevel::VeryLow => "Very low",
                ConfidenceLevel::Low => "Low",
                ConfidenceLevel::Normal => "Normal",
                ConfidenceLevel::High => "High",
                ConfidenceLevel::VeryHigh => "Very high",
            }
            .to_owned(),
        ),
        source_count: 2,
        restrictions: Vec::new(),
        has_more: false,
    }
}

/// Renders the ancestor chart: a known father (with a further-generation grandfather, so the fan
/// keeps going) and an unresearched mother slot.
fn ancestor_chart() -> Element {
    use_context_provider(NavState::new);
    use_context_provider(|| ChromeCtx(chrome("en")));
    let focus = node("I0001", "John Smith", ConfidenceLevel::Normal);
    let father = node("I0002", "Thomas Smith", ConfidenceLevel::High);
    let generations = vec![
        vec![
            PedigreeSlotVm::Known(father),
            PedigreeSlotVm::Unknown {
                hint: "mother of John Smith".to_owned(),
            },
        ],
        vec![
            PedigreeSlotVm::Unknown {
                hint: "father of Thomas Smith".to_owned(),
            },
            PedigreeSlotVm::Unknown {
                hint: "mother of Thomas Smith".to_owned(),
            },
        ],
    ];
    rsx! {
        AncestorTreeView { focus, generations }
    }
}

#[test]
fn ancestor_chart_renders_a_tree_of_treeitems_with_unknown_placeholders() {
    let mut vdom = VirtualDom::new(ancestor_chart);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains(r#"role="tree""#), "the chart is a tree:\n{html}");
    assert!(
        html.matches(r#"role="treeitem""#).count() >= 5,
        "focus + father + mother-unknown + 2 gen-2 slots are all treeitems:\n{html}"
    );
    assert!(html.contains("John Smith"), "the focus person renders:\n{html}");
    assert!(html.contains("Thomas Smith"), "the known father renders:\n{html}");
    assert!(
        html.contains("mother of John Smith"),
        "the unresearched slot names whose parent it is:\n{html}"
    );
    assert!(
        html.contains("father of Thomas Smith"),
        "a deeper unresearched slot still names whose parent it is:\n{html}"
    );
    assert!(
        html.contains(r#"data-level="high""#) && html.contains(">High"),
        "the known ancestor's confidence badge carries colour + text:\n{html}"
    );
    assert!(
        html.contains(r#"aria-expanded="true""#),
        "a non-leaf node is expandable:\n{html}"
    );
    assert!(html.contains(r#"aria-expanded="false""#), "a leaf node is not:\n{html}");
}

/// Renders the descendant chart: one child with a further grandchild.
fn descendant_chart() -> Element {
    use_context_provider(NavState::new);
    use_context_provider(|| ChromeCtx(chrome("en")));
    let focus = node("I0010", "Grand Parent", ConfidenceLevel::Normal);
    let mut child = node("I0011", "Mid Parent", ConfidenceLevel::VeryHigh);
    child.has_more = true;
    let grandchild = node("I0012", "Young Leaf", ConfidenceLevel::Low);
    let generations = vec![vec![child], vec![grandchild]];
    rsx! {
        DescendantTreeView { focus, generations }
    }
}

#[test]
fn descendant_chart_renders_a_tree_without_placeholder_padding() {
    let mut vdom = VirtualDom::new(descendant_chart);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    assert!(html.contains(r#"role="tree""#), "the chart is a tree:\n{html}");
    assert_eq!(
        html.matches(r#"role="treeitem""#).count(),
        3,
        "focus + child + grandchild only — no unknown padding for descendants:\n{html}"
    );
    assert!(html.contains("Grand Parent") && html.contains("Mid Parent") && html.contains("Young Leaf"));
    assert!(
        html.contains(r#"data-level="very-high""#),
        "the child's confidence badge:\n{html}"
    );
}

thread_local! {
    /// The language the view-switcher harness renders in — `VirtualDom::new` requires a bare
    /// no-argument root, so the language is smuggled in via a thread-local (the same trick
    /// `interpreter.rs` uses for its plugin form).
    static VIEW_SWITCHER_LANG: std::cell::Cell<&'static str> = const { std::cell::Cell::new("en") };
}

/// Renders the Pedigree tool's view switcher exactly as `PedigreeScreen` builds it, over the
/// localized `Chrome` labels for [`VIEW_SWITCHER_LANG`].
fn view_switcher() -> Element {
    let chrome = chrome(VIEW_SWITCHER_LANG.with(std::cell::Cell::get));
    let tabs = vec![
        TabItem {
            id: "pedigree".to_owned(),
            label: chrome.pedigree_view_label("pedigree"),
            count: None,
        },
        TabItem {
            id: "descendants".to_owned(),
            label: chrome.pedigree_view_label("descendants"),
            count: None,
        },
        TabItem {
            id: "relationships".to_owned(),
            label: chrome.pedigree_view_label("relationships"),
            count: None,
        },
    ];
    rsx! {
        Tabs { tabs, active: 0usize, aria_label: Some(chrome.pedigree_view_switcher_label()), onselect: move |_| {}, {rsx! {}} }
    }
}

#[test]
fn view_switcher_labels_are_localized_in_english() {
    VIEW_SWITCHER_LANG.with(|lang| lang.set("en"));
    let mut vdom = VirtualDom::new(view_switcher);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in [
        "role=\"tablist\"",
        "Pedigree",
        "Descendants",
        "Relationships",
        "Pedigree view",
    ] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}

#[test]
fn view_switcher_labels_are_localized_in_norwegian() {
    VIEW_SWITCHER_LANG.with(|lang| lang.set("no"));
    let mut vdom = VirtualDom::new(view_switcher);
    vdom.rebuild_in_place();
    let html = dioxus_ssr::render(&vdom);

    for needle in ["Etterkommere", "Slektskap"] {
        assert!(html.contains(needle), "expected {needle:?} in:\n{html}");
    }
}
