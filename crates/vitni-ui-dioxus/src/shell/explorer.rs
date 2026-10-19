//! The shell-level Explorer: the entity list column, shown only for an entity category.
//!
//! Hoisted out of the (now deleted) per-aggregate `*Screen`s so one component serves all 12
//! categories: it reads the active [`Category`], loads that category's list, and drives selection,
//! search, roving focus, and `[`/`]` stepping through the shared [`ListPane`] +
//! [`use_record_step`] — the only per-category variation
//! (the list intent, the chrome strings, the empty message) is a compact `match`. Selecting a row
//! opens it as a record tab in the editor host ([`NavState::open_record`]); the list stays put while
//! records open beside it (the VS Code model). Mounted by [`Shell`](crate::shell::root::Shell) only
//! when [`entity_category`] is `Some`, so a tool/Dashboard/Help destination shows no list at all.

use std::collections::BTreeSet;

use dioxus::prelude::*;
use vitni_app::ListQuery;
use vitni_ui::{ActiveFilter, Category, ListedRows, RecordRef, RowSort, RowVm, listed_kind};

use crate::app::AppCtx;
use crate::master_detail::{ListChrome, ListPane, SortChrome};
use crate::screens::use_record_step;
use crate::services::load_list_page;
use crate::shell::list_filters::{FilterBar, FilterEditing, FiltersButton};
use crate::shell::nav_state::{NavState, entity_category};

/// The Explorer column. Renders the active entity category's list, or nothing when the active
/// destination is not an entity category (a tool, the Dashboard, or Help — the guard also mirrors
/// [`Shell`](crate::shell::root::Shell)'s conditional mount, keeping this safe to render alone).
#[component]
pub fn Explorer() -> Element {
    let nav = try_consume_context::<NavState>();
    let Some(category) = nav.and_then(|nav| entity_category(*nav.active.read())) else {
        return rsx! {};
    };
    // Behind an open `SidePanel` the whole list column is inert, its rows and filter with it (#312).
    let behind_panel = nav.and_then(|nav| nav.panel_inert());
    // Keyed by category so switching categories remounts a fresh list (its own resource + signals),
    // mirroring how the old per-screen mount re-fetched on every category switch. `aside.list` is the
    // single grid cell of the `.app.has-explorer` layout (rail | list | work-area) — it must wrap the
    // list here (not inside `ExplorerList`) so the column exists even while the list is still loading.
    rsx! {
        aside { class: "list", "data-hook": "explorer", inert: behind_panel, aria_hidden: behind_panel,
            ExplorerList { key: "{category.id()}", category }
        }
    }
}

/// The list for one entity `category`, read from the store a page at a time (ADR 0052 §4): owns the
/// search, sort and filter signals that make up its [`ListQuery`], the [`ListedRows`] read so far, and
/// the `selected` signal. The first page is a resource over the query and
/// [`NavState::data_version`] (the rail-count idiom in `shell/root.rs`), so a new search, sort or
/// filter, or a create/edit/undo elsewhere, reads the list again from the top; the pages after it are
/// read as the window scrolls to them, and for a `[`/`]` step onto one. Selecting a row opens it in
/// [`NavState::open_record`], and the row highlight follows the active record tab (only when it
/// belongs to this category).
#[component]
fn ExplorerList(category: Category) -> Element {
    let AppCtx::Ready(state) = use_context::<AppCtx>() else {
        return rsx! {};
    };
    let Some(kind) = listed_kind(category) else {
        return rsx! {};
    };
    let mut nav = use_context::<NavState>();
    let services = state.services().clone();
    let chrome = state.chrome();
    let loc = state.data_loc();
    let entity = chrome.rail_label(category.label_id());
    let loading = chrome.loading();
    let list_chrome = ListChrome {
        list_label: entity.clone(),
        filter_placeholder: chrome.list_filter(&entity),
        empty: list_empty(loc, category),
        no_match: chrome.list_no_match(),
        sort: SortChrome::resolve(chrome),
    };
    let mut selected = use_signal(|| None::<String>);
    // Keep the row highlight in sync with the active record tab — but only when the active record is
    // in *this* category's list (a cross-category tab opened via a link leaves the list unhighlighted).
    use_effect(move || {
        selected.set(
            nav.active_record_ref()
                .filter(|record| record.category == category)
                .map(|record| record.human_id),
        );
    });
    let search = use_signal(String::new);
    let sort = use_signal(RowSort::default);
    let filters = use_signal(Vec::<ActiveFilter>::new);
    let editing = use_signal(|| None::<FilterEditing>);
    let query = use_memo(move || ListQuery {
        search: search(),
        filters: filters.read().iter().map(|active| active.filter.clone()).collect(),
        sort: sort().query_sort(kind),
        ..ListQuery::new(kind)
    });
    let mut rows = use_signal(ListedRows::default);
    let refused_rows = use_signal(ListedRows::default);
    // The query `rows` answers: a later page is read for it, never for a newer query whose first page
    // has not replaced `rows` yet, so one result never mixes rows from two queries.
    let mut held = use_signal(|| None::<ListQuery>);
    // Bumped whenever the list is read again from the top, so a later page read for the result it
    // replaced is dropped rather than mixed in.
    let mut generation = use_signal(|| 0_u64);
    let mut requested = use_signal(BTreeSet::<usize>::new);
    let first_services = services.clone();
    let first = use_resource(move || {
        let services = first_services.clone();
        let query = query();
        let _ = nav.data_version.read();
        async move {
            let page = load_list_page(services, query.clone(), 0).await;
            (query, page)
        }
    });
    use_effect(move || {
        if let Some((query, Ok(page))) = &*first.read() {
            let mut listed = ListedRows::default();
            listed.insert(page);
            *generation.write() += 1;
            requested.write().clear();
            held.set(Some(query.clone()));
            rows.set(listed);
        }
    });
    let fetch = use_callback(move |pages: Vec<usize>| {
        let reading = *generation.peek();
        let Some(query) = held.peek().clone() else {
            return;
        };
        for page in pages {
            if !requested.write().insert(page) {
                continue;
            }
            let services = services.clone();
            let query = query.clone();
            spawn(async move {
                let loaded = load_list_page(services, query, page).await;
                if *generation.peek() != reading {
                    return;
                }
                requested.write().remove(&page);
                match loaded {
                    Ok(page) => rows.write().insert(&page),
                    Err(error) => tracing::warn!(%error, page, "could not read a list page"),
                }
            });
        }
    });
    use_record_step(nav, category, rows, selected, fetch);
    let narrowed = !search.read().is_empty() || !filters.read().is_empty();
    // A query the store refuses keeps the toolbar, so the search or clause behind it can be changed.
    let refused = match &*first.read() {
        None if *generation.read() == 0 => return rsx! { p { class: "loading", "{loading}" } },
        Some((_, Err(message))) => Some(message.clone()),
        None | Some((_, Ok(_))) => None,
    };
    let (shown, chrome, narrowed) = match refused {
        Some(message) => (
            ReadSignal::from(refused_rows),
            ListChrome {
                empty: message,
                ..list_chrome
            },
            false,
        ),
        None => (ReadSignal::from(rows), list_chrome, narrowed),
    };
    rsx! {
        ListPane {
            rows: shown,
            search,
            sort,
            selected,
            chrome,
            narrowed,
            onselect: move |row: RowVm| nav.open_record(RecordRef {
                category,
                human_id: row.id,
                label: row.title,
            }),
            onmissing: fetch,
            tools: rsx! { FiltersButton { count: filters.read().len(), editing } },
            bar: rsx! { FilterBar { kind, filters, editing } },
        }
    }
}

/// The already-localized "no records yet" message for an entity category.
fn list_empty(loc: &vitni_ui::Localizer, category: Category) -> String {
    match category {
        Category::People | Category::Dashboard => loc.list_empty(),
        Category::Families => loc.family_list_empty(),
        Category::Events => loc.event_list_empty(),
        Category::Places => loc.place_list_empty(),
        Category::Sources => loc.source_list_empty(),
        Category::Citations => loc.citation_list_empty(),
        Category::Repositories => loc.repository_list_empty(),
        Category::Media => loc.media_list_empty(),
        Category::Notes => loc.note_list_empty(),
        Category::ResearchNotes => loc.research_note_list_empty(),
        Category::Tags => loc.tag_list_empty(),
        Category::DnaTests => loc.dna_test_list_empty(),
        Category::DnaMatches => loc.dna_match_list_empty(),
    }
}
