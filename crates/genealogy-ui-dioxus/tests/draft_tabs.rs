//! SSR-probe assertions for the draft-tab creation lifecycle and record reveal on [`NavState`]
//! (create = a draft tab).
//!
//! Like `dock.rs`, each probe provides a `NavState`, drives its methods in `use_hook`, and renders a
//! marker block the test inspects: `TABS:` is the open-tab count, `ACTIVE:` the active index (or
//! `NONE`), `KIND:` the active tab's kind (`DRAFT`/`SAVED`/`NONE`), `REF:` the active *saved* record
//! label (or `NONE`, so a draft reads `NONE`), and `DEST:` the active destination id.

use dioxus::prelude::*;
use genealogy_ui::{Category, Destination, RecordRef, Tool};
use genealogy_ui_dioxus::shell::nav_state::{NavState, OpenTab};

fn record(category: Category, human_id: &str, label: &str) -> RecordRef {
    RecordRef {
        category,
        human_id: human_id.to_owned(),
        label: label.to_owned(),
    }
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
