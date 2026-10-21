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
use std::rc::Rc;

use dioxus::prelude::*;
use vitni_app::{ListQuery, ListedKind, SavedSearchId, SavedSearchList};
use vitni_ui::{
    ActiveFilter, Category, ListLayout, ListedRows, RecordRef, RowVm, RowsPage, SavedNaming, StaleClauseVm, listed_kind,
};

use crate::app::{AppCtx, AppState};
use crate::master_detail::{ListChrome, ListPane};
use crate::screens::use_record_step;
use crate::services::{Services, load_list_page, load_saved_search, load_saved_searches};
use crate::shell::list_columns::{ColumnsMenu, ListMenu, SortMenu, persist_list_layout};
use crate::shell::list_filters::{FilterBar, FilterEditing, FiltersButton};
use crate::shell::nav_state::{NavState, entity_category};
use crate::shell::saved_menu::{
    ExplorerQuery, ResolvedSaved, SavedAction, SavedChange, SavedMenu, finish_saved_run, first_page_unless_marked,
};

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
/// search, filter and layout signals that make up its [`ListQuery`], the [`ListedRows`] read so far,
/// and the `selected` signal. The layout — the columns a row shows and the sort (ADR 0052 §7) — starts
/// from [`NavState::lists`], seeded from `workspace.toml`, and is written back there on each change, except a
/// layout a saved search applied, which belongs to the saved search.
/// The first page is a resource over the query and
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
    let filters = use_signal(Vec::<ActiveFilter>::new);
    let editing = use_signal(|| None::<FilterEditing>);
    let layout = use_signal(|| ListLayout::resolve(kind, nav.lists.peek().get(&kind)));
    let menu = use_signal(|| None::<ListMenu>);
    let list_query = ExplorerQuery {
        search,
        filters,
        stale: use_signal(Vec::<StaleClauseVm>::new),
        layout,
        applied_layout: use_signal(|| None::<ListLayout>),
        applied: use_signal(|| None),
        editing,
    };
    let stale = list_query.stale;
    let dir = services.dir.clone();
    // Written only when it differs from what is stored and is not a saved search's layout, so neither
    // mounting the list nor applying a saved search writes.
    use_effect(move || {
        let current = layout.read().clone();
        persist_list_layout(nav, &dir, &current, list_query.applied_layout);
    });
    let query = use_memo(move || {
        let layout = layout.read();
        ListQuery {
            search: search(),
            filters: filters.read().iter().map(|active| active.filter.clone()).collect(),
            sort: layout.sort.clone(),
            columns: Some(layout.columns.clone()),
            ..ListQuery::new(kind)
        }
    });
    let saved = use_saved_searches(&state, kind, list_query, menu, query);
    let PagedRows {
        rows,
        first,
        generation,
        fetch,
    } = use_paged_rows(&nav, services, query, stale, chrome.saved_marked_notice());
    use_record_step(nav, category, rows, selected, fetch);
    let narrowed = !search.read().is_empty() || !filters.read().is_empty();
    // A query the store refuses keeps the toolbar, so the search or clause behind it can be changed.
    let refused = match &*first.read() {
        None if *generation.read() == 0 => return rsx! { p { class: "loading", "{loading}" } },
        Some((_, Err(message))) => Some(message.clone()),
        None | Some((_, Ok(_))) => None,
    };
    let (chrome, narrowed) = match refused {
        Some(message) => (
            ListChrome {
                empty: message,
                ..list_chrome
            },
            false,
        ),
        None => (list_chrome, narrowed),
    };
    rsx! {
        ListPane {
            rows,
            search,
            sort: rsx! { SortMenu { layout, open: menu } },
            selected,
            chrome,
            narrowed,
            onselect: move |row: RowVm| nav.open_record(RecordRef {
                category,
                human_id: row.id,
                label: row.title,
            }),
            onmissing: fetch,
            tools: rsx! {
                FiltersButton { count: filters.read().len() + stale.read().len(), editing }
                ColumnsMenu { layout, open: menu }
                SavedMenu {
                    kind,
                    list: saved.list,
                    applied: list_query.applied,
                    open: menu,
                    naming: saved.naming,
                    onapply: saved.apply,
                    onact: saved.act,
                }
            },
            bar: rsx! { FilterBar { kind, filters, stale, editing } },
        }
    }
}

/// The rows of the list and how they are read: the first page for the current query, and later pages on
/// demand.
struct PagedRows {
    rows: Signal<ListedRows>,
    first: Resource<(ListQuery, Result<RowsPage, String>)>,
    generation: Signal<u64>,
    fetch: Callback<Vec<usize>>,
}

/// Reads the first page of `query` (unless a clause is marked, then `marked_notice` is the answer) and
/// the pages after it as they are asked for.
fn use_paged_rows(
    nav: &NavState,
    services: Services,
    query: Memo<ListQuery>,
    stale: Signal<Vec<StaleClauseVm>>,
    marked_notice: String,
) -> PagedRows {
    let nav = *nav;
    let mut rows = use_signal(ListedRows::default);
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
        let marked = !stale.read().is_empty();
        let marked_notice = marked_notice.clone();
        async move { first_page_unless_marked(services, query, marked, marked_notice).await }
    });
    use_effect(move || {
        let Some((query, result)) = &*first.read() else {
            return;
        };
        *generation.write() += 1;
        requested.write().clear();
        // A query that did not run leaves no rows, so stepping between records and reading further pages
        // never follow the list from before it.
        let mut listed = ListedRows::default();
        match result {
            Ok(page) => {
                listed.insert(page);
                held.set(Some(query.clone()));
            }
            Err(_) => held.set(None),
        }
        rows.set(listed);
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
    PagedRows {
        rows,
        first,
        generation,
        fetch,
    }
}

/// The Explorer's saved-search plumbing: what [`SavedMenu`] shows and the callbacks that apply and
/// change saved searches.
struct SavedControls {
    list: Signal<SavedSearchList>,
    naming: Signal<Option<SavedNaming>>,
    apply: Callback<SavedSearchId>,
    act: Callback<SavedAction>,
}

/// Wires saved searches to one list: the menu's entries (read each time it opens and after a change,
/// so one saved from the CLI shows), applying one (also for a palette handoff, taken on mount and as it
/// changes), and the changes the menu asks for.
fn use_saved_searches(
    state: &AppState,
    kind: ListedKind,
    list_query: ExplorerQuery,
    mut menu: Signal<Option<ListMenu>>,
    query: Memo<ListQuery>,
) -> SavedControls {
    let services = state.services();
    let (chrome, loc) = (state.chrome_rc(), state.data_loc_rc());
    let mut nav = use_context::<NavState>();
    let mut list = use_signal(SavedSearchList::default);
    let mut naming = use_signal(|| None::<SavedNaming>);
    let dir = services.dir.clone();
    let reload = use_callback(move |()| match load_saved_searches(&dir) {
        Ok(read) => list.set(read),
        Err(error) => tracing::warn!(%error, "could not read the saved searches"),
    });
    use_effect(move || {
        if menu() == Some(ListMenu::Saved) {
            reload.call(());
        }
    });
    let apply_services = services.clone();
    let mut latest_apply = use_signal(|| 0_u64);
    let apply = use_callback(move |id: SavedSearchId| {
        menu.set(None);
        let services = apply_services.clone();
        let this_apply = *latest_apply.peek() + 1;
        latest_apply.set(this_apply);
        spawn(async move {
            let loaded = load_saved_search(services, id, kind).await;
            if *latest_apply.peek() != this_apply {
                return;
            }
            let resolved = loaded.map(|(resolution, filters)| ResolvedSaved {
                id: resolution.search.id,
                name: resolution.search.name,
                query: resolution.query,
                filters,
            });
            finish_saved_run(nav, list_query, resolved);
        });
    });
    use_effect(move || {
        let Some(id) = *nav.saved_search_run.read() else {
            return;
        };
        nav.saved_search_run.set(None);
        apply.call(id);
    });
    let change_services = services.clone();
    let act = use_callback(move |action: SavedAction| {
        let change = SavedChange {
            nav,
            state: list_query,
            services: &change_services,
            query: &query.peek(),
            localizers: (Rc::clone(&chrome), Rc::clone(&loc)),
        };
        if change.perform(action) {
            naming.set(None);
            menu.set(None);
            reload.call(());
        }
    });
    SavedControls {
        list,
        naming,
        apply,
        act,
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
