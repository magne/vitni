//! SSR-probe assertions for the `⌘N`/[`NavState::request_new`] from-anywhere category picker
//! (issue #300): on an entity category it still opens a draft tab in place, unchanged; everywhere
//! else — the Dashboard, Help, every [`Tool`] — it raises [`Overlay::NewRecord`] instead of
//! silently doing nothing. Follows `shell.rs`'s probe shape for the tabstrip's own `NewRecordMenu`,
//! since [`NewRecordPicker`] reuses the same `Category::creatable()` listing over the shared `Modal`.

use dioxus::prelude::*;
use vitni_ui::{Category, Destination, Localizer, Tool};
use vitni_ui_dioxus::shell::nav_state::{NavState, Overlay};
use vitni_ui_dioxus::shell::new_record_picker::NewRecordPicker;
use vitni_ui_dioxus::shell::{ChromeCtx, DataLocCtx};

use std::rc::Rc;
use unic_langid::LanguageIdentifier;
use vitni_ui_dioxus::i18n::Chrome;

/// A chrome localizer for a single explicit language (deterministic for tests).
fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

/// A data localizer for a single explicit language — what names the picker's create items.
fn data_localizer(tag: &str) -> Rc<Localizer> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Localizer::with_languages(None, &[language]))
}

/// Renders a component to an HTML string.
fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

/// The marker block: whether the picker overlay is open, and how many record tabs are open.
fn probe(nav: &NavState) -> Element {
    let overlay_open = *nav.overlay.read() == Overlay::NewRecord;
    let tabs = nav.records.read().len();
    rsx! {
        div { "OVERLAY_OPEN:{overlay_open}" }
        div { "TABS:{tabs}" }
    }
}

/// `request_new` on an entity category (People) — must open a draft tab in place and never raise
/// the picker.
fn request_new_on_people() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.go_to(Destination::Category(Category::People));
        nav.request_new();
    });
    probe(&nav)
}

#[test]
fn request_new_on_an_entity_category_opens_a_draft_in_place() {
    let html = render(request_new_on_people);
    assert!(
        html.contains("OVERLAY_OPEN:false"),
        "no picker over an entity category:\n{html}"
    );
    assert!(html.contains("TABS:1"), "a draft tab opened in place:\n{html}");
}

/// `request_new` on the Dashboard — must raise the picker and open no tab.
fn request_new_on_dashboard() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.request_new());
    probe(&nav)
}

#[test]
fn request_new_on_the_dashboard_raises_the_picker() {
    let html = render(request_new_on_dashboard);
    assert!(
        html.contains("OVERLAY_OPEN:true"),
        "the picker opens from the Dashboard:\n{html}"
    );
    assert!(html.contains("TABS:0"), "no draft is opened:\n{html}");
}

/// `request_new` on a tool destination (Import) — must raise the picker and open no tab.
fn request_new_on_a_tool() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.go_to(Destination::Tool(Tool::Import));
        nav.request_new();
    });
    probe(&nav)
}

#[test]
fn request_new_on_a_tool_destination_raises_the_picker() {
    let html = render(request_new_on_a_tool);
    assert!(
        html.contains("OVERLAY_OPEN:true"),
        "the picker opens from a tool destination:\n{html}"
    );
    assert!(html.contains("TABS:0"), "no draft is opened:\n{html}");
}

/// `request_new` on Help — must raise the picker and open no tab.
fn request_new_on_help() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.go_to(Destination::Help { topic: None });
        nav.request_new();
    });
    probe(&nav)
}

#[test]
fn request_new_on_help_raises_the_picker() {
    let html = render(request_new_on_help);
    assert!(
        html.contains("OVERLAY_OPEN:true"),
        "the picker opens from Help:\n{html}"
    );
    assert!(html.contains("TABS:0"), "no draft is opened:\n{html}");
}

/// The picker rendered with no overlay open — must render nothing.
fn picker_closed() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    use_context_provider(NavState::new);
    rsx! {
        NewRecordPicker {}
    }
}

#[test]
fn the_picker_renders_nothing_while_closed() {
    let html = render(picker_closed);
    assert!(html.trim().is_empty(), "no overlay open, no markup:\n{html}");
}

/// The picker forced open via a seeded [`NavState`].
fn picker_open() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.overlay.set(Overlay::NewRecord));
    rsx! {
        NewRecordPicker {}
    }
}

#[test]
fn the_open_picker_is_a_modal_dialog_listing_every_creatable_category() {
    let html = render(picker_open);
    assert!(html.contains(r#"role="dialog""#), "picker dialog role:\n{html}");
    assert!(html.contains(r#"aria-modal="true""#), "picker is modal:\n{html}");
    assert_eq!(
        html.matches(r#"class="menu-item""#).count(),
        13,
        "one item per creatable category:\n{html}"
    );
    // Named by the record each item creates, so the picker and the pane it opens read the same.
    for label in [
        "New person",
        "New family",
        "New research note",
        "New DNA test",
        "New DNA match",
    ] {
        assert!(
            html.contains(&format!(">{label}<")),
            "expected create label {label:?}:\n{html}"
        );
    }
}

/// Picking a category from the picker — must open a draft there and close the overlay.
fn pick_families() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.overlay.set(Overlay::NewRecord);
        nav.request_new_for(Category::Families);
        nav.close_overlay();
    });
    probe(&nav)
}

#[test]
fn picking_a_category_opens_a_draft_there_and_closes_the_picker() {
    let html = render(pick_families);
    assert!(
        html.contains("OVERLAY_OPEN:false"),
        "the picker closes once a category is picked:\n{html}"
    );
    assert!(
        html.contains("TABS:1"),
        "a draft tab opened for the picked category:\n{html}"
    );
}

/// The picker forced open, localized to Norwegian.
fn picker_open_no() -> Element {
    use_context_provider(|| ChromeCtx(chrome("no")));
    use_context_provider(|| DataLocCtx(data_localizer("no")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.overlay.set(Overlay::NewRecord));
    rsx! {
        NewRecordPicker {}
    }
}

#[test]
fn the_picker_localizes_its_title_to_norwegian() {
    let html = render(picker_open_no);
    assert!(
        html.contains(r#"aria-label="Ny post""#),
        "a missing `no` key must fail this, not render the English fallback:\n{html}"
    );
    assert!(
        html.contains(">Ny person<"),
        "and the items name the record they create, with its Norwegian gender:\n{html}"
    );
    assert!(
        html.contains(">Nytt arkiv<"),
        "a neuter noun takes `Nytt`, which one shared prefix could never give it:\n{html}"
    );
}
