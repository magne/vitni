//! Host-free coverage of the Explorer's saved-search state transitions (#680, ADR 0052 §8-10).
//!
//! No SSR test can mount a live `ExplorerList` (`explorer.rs`), so the logic the
//! live components run is a set of free functions over [`NavState`] and the Explorer's signals,
//! driven here from a mount-time hook as `create_refresh.rs` does, with the result rendered as
//! markers. The marked-clause guards (no rows read, no save or update) are tested on the functions the
//! components call. What only a live webview shows — applying fills the search box and the chips — is
//! covered by the `saved-searches` gui-pass scenario alone (T8), not here.

#![expect(clippy::expect_used, reason = "a clause that does not apply aborts the test")]

use dioxus::prelude::*;
use uuid::Uuid;
use vitni_app::{FilterOp, ListQuery, ListSort, ListedKind, SavedSearchId, SortDirection};
use vitni_ui::{
    ActiveFilter, Category, Destination, FilterDraft, ListLayout, SavedFilters, StaleClauseVm, saved_layout,
};
use vitni_ui_dioxus::i18n::Chrome;
use vitni_ui_dioxus::shell::list_columns::persist_list_layout;
use vitni_ui_dioxus::shell::list_filters::FilterEditing;
use vitni_ui_dioxus::shell::nav_state::NavState;
use vitni_ui_dioxus::shell::palette::run_saved_search;
use vitni_ui_dioxus::shell::saved_menu::{
    ExplorerQuery, ResolvedSaved, SavedAction, SavedChange, finish_saved_run, first_page_unless_marked,
};

fn render(app: fn() -> Element) -> String {
    let mut vdom = VirtualDom::new(app);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn id(n: u128) -> SavedSearchId {
    SavedSearchId::from_uuid(Uuid::from_u128(n))
}

fn birth_between() -> ActiveFilter {
    let kind = ListedKind::Person;
    let mut draft = FilterDraft::new(kind)
        .with_column(kind, "birth")
        .with_op(FilterOp::Between);
    "1800".clone_into(&mut draft.first);
    "1850".clone_into(&mut draft.second);
    draft.apply(kind).expect("applies")
}

fn explorer_query() -> ExplorerQuery {
    ExplorerQuery {
        search: use_signal(String::new),
        filters: use_signal(Vec::new),
        stale: use_signal(Vec::new),
        layout: use_signal(|| ListLayout::resolve(ListedKind::Person, None)),
        applied_layout: use_signal(|| None),
        applied: use_signal(|| None),
        editing: use_signal(|| None),
    }
}

fn marker(state: &ExplorerQuery) -> Element {
    let search = state.search.read().clone();
    let filters = state.filters.read().len();
    let stale = state.stale.read().len();
    let columns = state.layout.read().columns.join(",");
    let sort = state.layout.read().sort.as_ref().map_or_else(
        || "NONE".to_owned(),
        |sort| format!("{}:{:?}", sort.column, sort.direction),
    );
    let remembered = state.applied_layout.read().as_ref() == Some(&*state.layout.read());
    let applied = state
        .applied
        .read()
        .as_ref()
        .map_or_else(|| "NONE".to_owned(), |(_, name)| name.clone());
    rsx! {
        div { "SEARCH:{search}" }
        div { "FILTERS:{filters}" }
        div { "STALE:{stale}" }
        div { "COLUMNS:{columns}" }
        div { "SORT:{sort}" }
        div { "REMEMBERED:{remembered}" }
        div { "APPLIED:{applied}" }
    }
}

fn saved_query() -> ListQuery {
    ListQuery {
        search: "Fana".to_owned(),
        sort: Some(ListSort {
            column: "birth".to_owned(),
            direction: SortDirection::Descending,
        }),
        columns: Some(vec!["death".to_owned(), "sex".to_owned()]),
        ..ListQuery::new(ListedKind::Person)
    }
}

fn resolved(stale: Vec<StaleClauseVm>) -> ResolvedSaved {
    ResolvedSaved {
        id: id(1),
        name: "Fana births".to_owned(),
        query: saved_query(),
        filters: SavedFilters {
            applied: vec![birth_between()],
            stale,
        },
    }
}

fn apply_readable() -> Element {
    let nav = use_context_provider(NavState::new);
    let state = explorer_query();
    use_hook(move || finish_saved_run(nav, state, Ok(resolved(Vec::new()))));
    marker(&state)
}

#[test]
fn applying_a_saved_search_sets_search_filters_layout_and_the_applied_name() {
    let html = render(apply_readable);

    for needle in [
        "SEARCH:Fana",
        "FILTERS:1",
        "STALE:0",
        "COLUMNS:death,sex",
        "SORT:birth:Descending",
        "APPLIED:Fana births",
    ] {
        assert!(html.contains(needle), "expected {needle:?}:\n{html}");
    }
    assert!(
        html.contains("REMEMBERED:true"),
        "the applied layout is remembered so it is not persisted:\n{html}"
    );
}

fn apply_while_editing() -> Element {
    let nav = use_context_provider(NavState::new);
    let mut state = explorer_query();
    use_hook(move || {
        state.editing.set(Some(FilterEditing::Clause(0)));
        finish_saved_run(nav, state, Ok(resolved(Vec::new())));
    });
    let editing = state.editing.read().is_some();
    rsx! { div { "EDITING:{editing}" } }
}

#[test]
fn applying_a_saved_search_closes_the_filter_editor() {
    let html = render(apply_while_editing);
    assert!(
        html.contains("EDITING:false"),
        "the editor targets the old clauses:
{html}"
    );
}

fn apply_marked() -> Element {
    let nav = use_context_provider(NavState::new);
    let state = explorer_query();
    use_hook(move || {
        let stale = vec![StaleClauseVm {
            column: "shoe size".to_owned(),
            label: "shoe size is not a column".to_owned(),
        }];
        finish_saved_run(nav, state, Ok(resolved(stale)));
    });
    marker(&state)
}

#[test]
fn a_marked_clause_is_kept_beside_the_readable_ones() {
    let html = render(apply_marked);

    assert!(
        html.contains("STALE:1"),
        "the marked clause is held for the filter bar:\n{html}"
    );
    assert!(html.contains("FILTERS:1"), "the readable clause is applied:\n{html}");
}

fn apply_twice() -> Element {
    let nav = use_context_provider(NavState::new);
    let state = explorer_query();
    use_hook(move || {
        let stale = vec![StaleClauseVm {
            column: "shoe size".to_owned(),
            label: "gone".to_owned(),
        }];
        finish_saved_run(nav, state, Ok(resolved(stale)));
        finish_saved_run(nav, state, Ok(resolved(Vec::new())));
    });
    marker(&state)
}

#[test]
fn applying_another_search_replaces_the_marked_clauses() {
    let html = render(apply_twice);

    assert!(html.contains("STALE:0"), "the earlier marks do not linger:\n{html}");
    assert!(
        html.contains("FILTERS:1"),
        "and the filters are replaced, not appended:\n{html}"
    );
}

fn missing_handoff() -> Element {
    let nav = use_context_provider(NavState::new);
    let state = explorer_query();
    use_hook(move || {
        finish_saved_run(nav, state, Err("No saved search `gone`".to_owned()));
    });
    let notice = nav
        .notice
        .read()
        .as_ref()
        .map_or_else(|| "NONE".to_owned(), |notice| notice.message.clone());
    rsx! {
        div { "NOTICE:{notice}" }
        {marker(&state)}
    }
}

#[test]
fn a_handoff_whose_search_is_gone_raises_a_notice_and_applies_nothing() {
    let html = render(missing_handoff);

    assert!(
        html.contains("NOTICE:No saved search `gone`"),
        "the not-found notice:\n{html}"
    );
    for needle in ["SEARCH:<", "FILTERS:0", "STALE:0", "APPLIED:NONE", "REMEMBERED:false"] {
        assert!(html.contains(needle), "nothing applied, expected {needle:?}:\n{html}");
    }
}

fn run_from_palette() -> Element {
    let mut nav = use_context_provider(NavState::new);
    use_hook(move || run_saved_search(&mut nav, id(7), Category::Places));
    let handoff = nav
        .saved_search_run
        .read()
        .map_or_else(|| "NONE".to_owned(), |run| run.to_string());
    let active = format!("{:?}", *nav.active.read());
    rsx! {
        div { "HANDOFF:{handoff}" }
        div { "ACTIVE:{active}" }
    }
}

#[test]
fn running_a_saved_search_sets_the_handoff_and_opens_its_category() {
    let html = render(run_from_palette);

    assert!(
        html.contains(&format!("HANDOFF:{}", id(7))),
        "the id waits for the list to take it:\n{html}"
    );
    assert!(
        html.contains(&format!("ACTIVE:{:?}", Destination::Category(Category::Places))),
        "the kind's list is shown:\n{html}"
    );
}

fn persist(applied: bool) -> Element {
    let nav = use_context_provider(NavState::new);
    let dir = use_hook(|| std::rc::Rc::new(tempfile::tempdir().expect("tempdir")));
    use_hook(|| {
        let saved = saved_layout(&saved_query());
        let applied_layout = Signal::new(applied.then(|| saved.clone()));
        persist_list_layout(nav, dir.path(), &saved, applied_layout);
    });
    let kinds = nav.lists.read().len();
    rsx! { div { "WRITTEN:{kinds}" } }
}

fn persist_applied() -> Element {
    persist(true)
}

fn persist_user_change() -> Element {
    persist(false)
}

#[test]
fn an_applied_layout_is_not_persisted() {
    let html = render(persist_applied);

    assert!(
        html.contains("WRITTEN:0"),
        "applying a saved search writes no list preferences:\n{html}"
    );
}

#[test]
fn a_layout_the_user_changed_is_persisted() {
    let html = render(persist_user_change);

    assert!(
        html.contains("WRITTEN:1"),
        "a layout that is neither stored nor applied is kept:\n{html}"
    );
}

fn persist_back_to_applied() -> Element {
    let nav = use_context_provider(NavState::new);
    let dir = use_hook(|| std::rc::Rc::new(tempfile::tempdir().expect("tempdir")));
    use_hook(|| {
        let saved = saved_layout(&saved_query());
        let applied_layout = Signal::new(Some(saved.clone()));
        let mut changed = saved.clone();
        changed.toggle("birth");
        persist_list_layout(nav, dir.path(), &changed, applied_layout);
        persist_list_layout(nav, dir.path(), &saved, applied_layout);
    });
    let columns = nav
        .lists
        .read()
        .get(&ListedKind::Person)
        .and_then(|preferences| preferences.columns.clone())
        .map_or_else(String::new, |columns| columns.join(","));
    rsx! { div { "STORED:{columns}" } }
}

fn persist_back_to_applied_via_stored() -> Element {
    let nav = use_context_provider(NavState::new);
    let dir = use_hook(|| std::rc::Rc::new(tempfile::tempdir().expect("tempdir")));
    use_hook(|| {
        let saved = saved_layout(&saved_query());
        let stored = ListLayout::resolve(ListedKind::Person, None);
        let applied_layout = Signal::new(Some(saved.clone()));
        persist_list_layout(nav, dir.path(), &stored, applied_layout);
        persist_list_layout(nav, dir.path(), &saved, applied_layout);
    });
    let columns = nav
        .lists
        .read()
        .get(&ListedKind::Person)
        .and_then(|preferences| preferences.columns.clone())
        .map_or_else(String::new, |columns| columns.join(","));
    rsx! { div { "STORED:{columns}" } }
}

#[test]
fn returning_to_the_applied_layout_after_the_stored_one_is_persisted() {
    let html = render(persist_back_to_applied_via_stored);

    assert!(
        html.contains("STORED:death,sex<"),
        "the layout the user came back to is the last choice, stored:\n{html}"
    );
}

#[test]
fn returning_to_the_applied_layout_after_a_change_is_persisted() {
    let html = render(persist_back_to_applied);

    assert!(
        html.contains("STORED:death,sex<"),
        "the layout the user came back to is the last choice, stored:\n{html}"
    );
}

fn services_over_missing_workspace() -> vitni_ui_dioxus::services::Services {
    let config: vitni_app::Config = serde_json::from_value(serde_json::json!({
        "operator": { "id": "00000000-0000-0000-0000-000000000001", "display": "Ada", "email": null }
    }))
    .expect("config");
    vitni_ui_dioxus::services::Services {
        data_quality: vitni_ui_dioxus::services::DataQualityCache::default(),
        config,
        dir: std::path::PathBuf::from("/nonexistent/vitni-saved-search-test"),
        open_workspace: "ws".to_owned(),
        host: std::rc::Rc::new(vitni_plugin_host::PluginHost::new().expect("host")),
        plugins_dir: vitni_app::embedded_plugins_dir(),
    }
}

#[tokio::test]
async fn a_marked_clause_stops_the_list_from_reading_rows() {
    let query = ListQuery::new(ListedKind::Person);

    let (held, marked) = first_page_unless_marked(
        services_over_missing_workspace(),
        query.clone(),
        true,
        "MARKED".to_owned(),
    )
    .await;
    let (_, unmarked) = first_page_unless_marked(
        services_over_missing_workspace(),
        query.clone(),
        false,
        "MARKED".to_owned(),
    )
    .await;

    assert_eq!(held, query);
    assert_eq!(
        marked.err().as_deref(),
        Some("MARKED"),
        "a marked search shows the notice"
    );
    assert_ne!(
        unmarked.err().as_deref(),
        Some("MARKED"),
        "an unmarked one reads the store (and fails here, on the missing workspace)"
    );
}

fn save_while_marked(action: SavedAction) -> String {
    let mut vdom = VirtualDom::new_with_props(save_while_marked_app, action);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

fn save_while_marked_app(action: SavedAction) -> Element {
    let nav = use_context_provider(NavState::new);
    let state = explorer_query();
    let mut done = use_signal(|| true);
    use_hook(move || {
        let mut state = state;
        state.stale.set(vec![StaleClauseVm {
            column: "shoe size".to_owned(),
            label: "shoe size is not a column".to_owned(),
        }]);
        let services = services_over_missing_workspace();
        let chrome = std::rc::Rc::new(Chrome::with_languages(None, &["en".parse().expect("tag")]));
        let query = ListQuery::new(ListedKind::Person);
        let change = SavedChange {
            nav,
            state,
            services: &services,
            query: &query,
            localizers: (chrome, vitni_ui_dioxus::shell::data_loc()),
        };
        done.set(change.perform(action.clone()));
    });
    let notice = nav
        .notice
        .read()
        .as_ref()
        .map_or_else(|| "NONE".to_owned(), |notice| notice.message.clone());
    rsx! { div { "DONE:{done} NOTICE:{notice}" } }
}

#[test]
fn save_as_new_and_update_are_refused_while_a_clause_is_marked() {
    let blocked = Chrome::with_languages(None, &["en".parse().expect("tag")]).saved_marked_blocks_save();
    for action in [SavedAction::SaveNew("Fana".to_owned()), SavedAction::Update] {
        let html = save_while_marked(action);
        assert!(
            html.contains(&format!("NOTICE:{blocked}")),
            "expected the refusal:\n{html}"
        );
    }
}

fn services_in(dir: &std::path::Path) -> vitni_ui_dioxus::services::Services {
    vitni_ui_dioxus::services::Services {
        dir: dir.to_path_buf(),
        ..services_over_missing_workspace()
    }
}

#[derive(Clone, PartialEq)]
struct PerformProps {
    dir: std::path::PathBuf,
    action: SavedAction,
    query: ListQuery,
    applied: Option<SavedSearchId>,
}

fn perform_app(props: PerformProps) -> Element {
    let nav = use_context_provider(NavState::new);
    let state = explorer_query();
    let mut done = use_signal(|| false);
    use_hook(move || {
        let mut state = state;
        if let Some(id) = props.applied {
            state.applied.set(Some((id, "Fana".to_owned())));
        }
        let services = services_in(&props.dir);
        let chrome = std::rc::Rc::new(Chrome::with_languages(None, &["en".parse().expect("tag")]));
        let change = SavedChange {
            nav,
            state,
            services: &services,
            query: &props.query,
            localizers: (chrome, vitni_ui_dioxus::shell::data_loc()),
        };
        done.set(change.perform(props.action.clone()));
    });
    let applied_name = state
        .applied
        .read()
        .as_ref()
        .map_or_else(|| "NONE".to_owned(), |(_, name)| name.clone());
    rsx! { div { "DONE:{done} APPLIED:{applied_name}" } }
}

fn perform_in(dir: &std::path::Path, action: SavedAction, query: &ListQuery, applied: Option<SavedSearchId>) -> bool {
    perform_html(dir, action, query, applied).contains("DONE:true")
}

fn perform_html(
    dir: &std::path::Path,
    action: SavedAction,
    query: &ListQuery,
    applied: Option<SavedSearchId>,
) -> String {
    let props = PerformProps {
        dir: dir.to_path_buf(),
        action,
        query: query.clone(),
        applied,
    };
    let mut vdom = VirtualDom::new_with_props(perform_app, props);
    vdom.rebuild_in_place();
    dioxus_ssr::render(&vdom)
}

#[test]
fn update_writes_the_list_query_over_the_applied_saved_search() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("workspace.toml"),
        "database_url = \"sqlite:vitni.db\"\n",
    )
    .expect("manifest");
    let services = services_in(dir.path());
    let original = ListQuery {
        search: "Original".to_owned(),
        ..ListQuery::new(ListedKind::Person)
    };
    let id = vitni_ui_dioxus::services::save_new_saved_search(&services, "Fana", &original).expect("saved");
    let narrowed = ListQuery {
        search: "Narrowed".to_owned(),
        ..ListQuery::new(ListedKind::Person)
    };

    assert!(perform_in(dir.path(), SavedAction::Update, &narrowed, Some(id)));

    let stored = std::fs::read_to_string(dir.path().join("workspace.toml")).expect("manifest");
    assert!(stored.contains("Narrowed"), "the new query is stored:\n{stored}");
    assert!(!stored.contains("Original"), "the old query is replaced:\n{stored}");
}

#[test]
fn rename_makes_the_applied_saved_search_carry_the_new_name() {
    let dir = tempfile::tempdir().expect("tempdir");
    std::fs::write(
        dir.path().join("workspace.toml"),
        "database_url = \"sqlite:vitni.db\"\n",
    )
    .expect("manifest");
    let services = services_in(dir.path());
    let query = ListQuery::new(ListedKind::Person);
    let id = vitni_ui_dioxus::services::save_new_saved_search(&services, "Fana", &query).expect("saved");

    let html = perform_html(dir.path(), SavedAction::Rename("Olsens".to_owned()), &query, Some(id));

    assert!(html.contains("DONE:true"), "the rename succeeds:\n{html}");
    assert!(
        html.contains("APPLIED:Olsens"),
        "the applied search follows the rename:\n{html}"
    );
}
