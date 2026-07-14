//! Coverage for the shell-level Explorer and the two shell shapes.
//!
//! `entity_category` is a pure fn (the single source of truth for `rail | Explorer | editor` vs the
//! full-width `rail | screen`), so it is unit-tested directly. The Explorer component's absence on a
//! non-entity destination and the draft-tab rendering in the record tabstrip are SSR probes (the same
//! host-free pattern as `dock.rs`): a live list needs a real workspace, so these assert the negative
//! (no list for a tool) and the draft chrome, not a populated list.

use std::rc::Rc;

use dioxus::prelude::*;
use genealogy_ui::{Category, Destination, Tool};
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::shell::ChromeCtx;
use genealogy_ui_dioxus::shell::explorer::Explorer;
use genealogy_ui_dioxus::shell::nav_state::{NavState, entity_category};
use genealogy_ui_dioxus::shell::tabstrip::RecordTabstrip;
use unic_langid::LanguageIdentifier;

fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn entity_category_is_some_only_for_the_twelve_aggregates() {
    // The Dashboard is a `Category` variant but a full-width destination — never an entity workspace.
    assert_eq!(entity_category(Destination::Category(Category::Dashboard)), None);
    // Every other category drives the Explorer + editor layout.
    for category in Category::all() {
        let expected = if category == Category::Dashboard {
            None
        } else {
            Some(category)
        };
        assert_eq!(
            entity_category(Destination::Category(category)),
            expected,
            "{category:?}"
        );
    }
    // Tools and Help are full-width, no Explorer.
    for tool in Tool::all() {
        assert_eq!(entity_category(Destination::Tool(tool)), None, "{tool:?}");
    }
    assert_eq!(entity_category(Destination::Help { topic: None }), None);
}

/// The Explorer on a tool destination: it reads its guard and renders nothing (no list column).
fn explorer_on_tool() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.go_to(Destination::Tool(Tool::Pedigree)));
    rsx! {
        Explorer {}
    }
}

#[test]
fn explorer_renders_nothing_for_a_tool() {
    let html = render(explorer_on_tool);
    assert!(
        !html.contains(r#"role="listbox""#),
        "no entity list is shown for a tool destination:\n{html}"
    );
}

/// The record tabstrip after opening a person draft: the draft tab is active, carries the `draft`
/// class, shows the localized "New <entity>" label, and is not a drag source.
fn tabstrip_with_draft() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.open_create(Category::People));
    rsx! {
        RecordTabstrip {}
    }
}

#[test]
fn a_draft_tab_renders_its_own_chrome() {
    let html = render(tabstrip_with_draft);
    assert!(
        html.contains(r#"class="rtab active draft""#),
        "the draft tab is active and marked as a draft:\n{html}"
    );
    assert!(
        html.contains("New People"),
        "the draft tab shows its localized label:\n{html}"
    );
    assert!(
        html.contains(r#"draggable="false""#),
        "a draft tab is not a drag source (it has no record to dock):\n{html}"
    );
}
