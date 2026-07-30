//! SSR-probe assertions for the draft-tab creation lifecycle, record reveal, and the tabstrip's
//! unsaved marker on [`NavState`] (create = a draft tab).
//!
//! Like `dock.rs`, each probe provides a `NavState`, drives its methods in `use_hook`, and renders a
//! marker block the test inspects: `TABS:` is the open-tab count, `ACTIVE:` the active index (or
//! `NONE`), `KIND:` the active tab's kind (`DRAFT`/`SAVED`/`NONE`), `REF:` the active *saved* record
//! label (or `NONE`, so a draft reads `NONE`), and `DEST:` the active destination id. The marker
//! probes render the real [`RecordTabstrip`], so they need a `ChromeCtx` too.

use std::rc::Rc;

use dioxus::prelude::*;
use genealogy_ui::{Category, Destination, ProvenanceDraft, RecordRef, TagDraft, Tool};
use genealogy_ui_dioxus::i18n::Chrome;
use genealogy_ui_dioxus::shell::ChromeCtx;
use genealogy_ui_dioxus::shell::nav_state::{EditKey, NavState, OpenTab, StashedEdit};
use genealogy_ui_dioxus::shell::tabstrip::RecordTabstrip;
use unic_langid::LanguageIdentifier;

/// A chrome localizer for a single explicit language (deterministic for tests).
fn chrome(tag: &str) -> Rc<Chrome> {
    let language = tag.parse::<LanguageIdentifier>().unwrap_or_default();
    Rc::new(Chrome::with_languages(None, &[language]))
}

fn record(category: Category, human_id: &str, label: &str) -> RecordRef {
    RecordRef {
        category,
        human_id: human_id.to_owned(),
        label: label.to_owned(),
    }
}

/// Parks an in-progress edit of the saved record `(category, human_id)` in the shell, the way a
/// mid-edit detail pane does — what the tabstrip's unsaved marker reads.
fn mark_dirty(nav: &mut NavState, category: Category, human_id: &str) {
    let draft = TagDraft {
        name: "edited".to_owned(),
        ..TagDraft::new()
    };
    nav.stash_edit(
        EditKey::saved(category, human_id),
        StashedEdit::new(draft, TagDraft::new(), ProvenanceDraft::default()),
    );
}

fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn dest_id(destination: Destination) -> String {
    match destination {
        Destination::Category(category) => category.id().to_owned(),
        Destination::Tool(tool) => tool.id().to_owned(),
        Destination::Help { .. } => "help".to_owned(),
    }
}

fn probe(nav: &NavState) -> Element {
    let tabs = nav.records.read().len();
    let active = nav
        .active_record
        .read()
        .map_or_else(|| "NONE".to_owned(), |index| index.to_string());
    let kind = match nav.active_tab() {
        Some(OpenTab::Draft(_)) => "DRAFT",
        Some(OpenTab::Saved(_)) => "SAVED",
        None => "NONE",
    };
    let reference = nav
        .active_record_ref()
        .map_or_else(|| "NONE".to_owned(), |record| record.label);
    let dest = dest_id(*nav.active.read());
    rsx! {
        div { "TABS:{tabs}" }
        div { "ACTIVE:{active}" }
        div { "KIND:{kind}" }
        div { "REF:{reference}" }
        div { "DEST:{dest}" }
    }
}

fn open_one_draft() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.open_create(Category::People));
    probe(&nav)
}

fn open_draft_twice_same_category() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_create(Category::People);
        nav.open_create(Category::People);
    });
    probe(&nav)
}

fn commit_replaces_draft_in_place() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record(Category::People, "I0001", "Ada"));
        nav.open_create(Category::Families);
        nav.commit_draft(record(Category::Families, "F0001", "Bell family"));
    });
    probe(&nav)
}

fn cancel_closes_the_draft() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record(Category::People, "I0001", "Ada"));
        nav.open_create(Category::Families);
        nav.cancel_draft(Category::Families);
    });
    probe(&nav)
}

fn reveal_from_tool_switches_category() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.go_to(Destination::Tool(Tool::Merge));
        nav.reveal_record(record(Category::People, "I0001", "Ada"));
    });
    probe(&nav)
}

fn reveal_within_category_keeps_list() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.go_to(Destination::Category(Category::People));
        nav.reveal_record(record(Category::Events, "E0001", "Birth"));
    });
    probe(&nav)
}

#[test]
fn open_create_opens_an_active_draft_tab() {
    let html = render(open_one_draft);
    assert!(html.contains("TABS:1"), "one tab is open:\n{html}");
    assert!(html.contains("KIND:DRAFT"), "the active tab is a draft:\n{html}");
    assert!(html.contains("REF:NONE"), "a draft has no saved record:\n{html}");
}

#[test]
fn open_create_twice_focuses_the_existing_draft() {
    let html = render(open_draft_twice_same_category);
    assert!(html.contains("TABS:1"), "at most one draft per category:\n{html}");
}

#[test]
fn commit_draft_replaces_the_draft_in_place() {
    let html = render(commit_replaces_draft_in_place);
    assert!(
        html.contains("TABS:2"),
        "the draft became the saved tab, not a new one:\n{html}"
    );
    assert!(
        html.contains("ACTIVE:1"),
        "the committed record keeps the draft's slot:\n{html}"
    );
    assert!(html.contains("KIND:SAVED"), "the tab is now a saved record:\n{html}");
    assert!(html.contains("REF:Bell family"), "the saved label shows:\n{html}");
}

#[test]
fn cancel_draft_closes_the_tab() {
    let html = render(cancel_closes_the_draft);
    assert!(html.contains("TABS:1"), "cancelling drops the draft tab:\n{html}");
    assert!(
        html.contains("KIND:SAVED"),
        "the remaining tab is the saved record:\n{html}"
    );
}

#[test]
fn reveal_from_a_tool_reveals_the_record_category() {
    let html = render(reveal_from_tool_switches_category);
    assert!(
        html.contains("DEST:people"),
        "revealing from a tool switches to the category:\n{html}"
    );
    assert!(html.contains("REF:Ada"), "the record opens:\n{html}");
}

/// The tabstrip with Ada holding an unsaved edit and Bob clean.
fn tabstrip_with_one_dirty_record() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record(Category::People, "I0001", "Ada"));
        nav.open_record(record(Category::People, "I0002", "Bob"));
        mark_dirty(&mut nav, Category::People, "I0001");
    });
    rsx! {
        RecordTabstrip {}
    }
}

#[test]
fn a_dirty_saved_tab_carries_the_unsaved_class_and_glyph() {
    // The whole point of the marker: an unsaved edit is visible without opening the tab. Colour alone
    // would fail WCAG 1.4.1, so the glyph and the accessible name carry it too.
    let html = render(tabstrip_with_one_dirty_record);
    assert!(
        html.contains(r#"class="rtab unsaved""#),
        "the dirty (inactive) tab is marked unsaved:\n{html}"
    );
    assert!(
        html.contains(r#"class="unsaved-dot" aria-hidden="true""#),
        "the marker renders a glyph, not colour alone:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Ada — unsaved changes""#),
        "the dirty tab's accessible name says so:\n{html}"
    );
    assert_eq!(
        html.matches("unsaved-dot").count(),
        1,
        "only the dirty tab gets a marker — Bob is clean:\n{html}"
    );
    assert!(
        html.contains(r#"class="rtab active""#),
        "the clean active tab keeps its plain class:\n{html}"
    );
}

/// The tabstrip with both open records holding an unsaved edit (issue #239: the edits are parked per
/// record in the shell, so both are visible at once even though only one pane is mounted).
fn tabstrip_with_two_dirty_records() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || {
        nav.open_record(record(Category::People, "I0001", "Ada"));
        nav.open_record(record(Category::People, "I0002", "Bob"));
        mark_dirty(&mut nav, Category::People, "I0001");
        mark_dirty(&mut nav, Category::People, "I0002");
    });
    rsx! {
        RecordTabstrip {}
    }
}

#[test]
fn every_tab_holding_an_unsaved_edit_carries_the_marker_not_just_the_active_one() {
    let html = render(tabstrip_with_two_dirty_records);
    assert_eq!(
        html.matches("unsaved-dot").count(),
        2,
        "both dirty tabs show the glyph:\n{html}"
    );
    assert!(
        html.contains(r#"class="rtab unsaved""#) && html.contains(r#"class="rtab active unsaved""#),
        "the inactive and the active dirty tab are both marked:\n{html}"
    );
    assert!(
        html.contains(r#"aria-label="Ada — unsaved changes""#)
            && html.contains(r#"aria-label="Bob — unsaved changes""#),
        "each dirty tab's accessible name says so:\n{html}"
    );
}

/// The tabstrip with a single draft tab open (nothing saved yet).
fn tabstrip_with_draft_only() -> Element {
    use_context_provider(|| ChromeCtx(chrome("en")));
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || nav.open_create(Category::People));
    rsx! {
        RecordTabstrip {}
    }
}

#[test]
fn a_draft_tab_carries_the_unsaved_marker_too() {
    let html = render(tabstrip_with_draft_only);
    assert!(
        html.contains(r#"class="rtab active draft unsaved""#),
        "a draft is unsaved by definition and gets the same marker:\n{html}"
    );
    assert!(html.contains("unsaved-dot"), "the draft tab shows the glyph:\n{html}");
}

#[test]
fn reveal_within_a_category_leaves_the_list() {
    let html = render(reveal_within_category_keeps_list);
    assert!(
        html.contains("DEST:people"),
        "opening a cross-category link keeps the current list:\n{html}"
    );
    assert!(
        html.contains("REF:Birth"),
        "the linked record still opens as the active tab:\n{html}"
    );
}
