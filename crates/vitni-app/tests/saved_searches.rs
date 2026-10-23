//! Saved-search use-cases (ADR 0052 §8-10): save, list, update, rename and delete over the
//! workspace manifest, with the `Session` minting ids and unreadable entries passed through.

#![expect(clippy::expect_used, clippy::panic, reason = "tests abort on setup failure")]

use std::path::Path;

use uuid::Uuid;
use vitni_app::saved_search::{SavedSearchEntry, SavedSearchId};
use vitni_app::{
    AppDefaults, AppError, ConfigStore, FileConfigStore, FilterOp, FilterValue, ListFilter, ListPage, ListQuery,
    ListQueryError, ListedKind, MutationMeta, NewPerson, NewPlace, OperatorConfig, PersonNameParts, Provenance,
    SavedSearchError, SavedSearchResolution, Session, Workspace, WorkspaceDefaults, create_person, create_place,
    create_tag, delete_saved_search, list_persons_page, list_places_page, list_saved_searches, rename_saved_search,
    resolve_saved_search, resolve_saved_search_id, save_saved_search, tag_person, update_saved_search,
};
use vitni_core::enums::EvidenceLevel;
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind};

const UNREADABLE: &str =
    "\n[[saved_searches]]\nname = \"Broken\"\nid = 42\n[saved_searches.query]\nkind = \"galaxy\"\n";

fn operator() -> OperatorConfig {
    OperatorConfig {
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
        email: None,
    }
}

fn session() -> Session {
    Session::new(Agent {
        kind: AgentKind::Human,
        id: AgentId::from_uuid(Uuid::from_u128(1)),
        display: Some("Tester".to_owned()),
    })
}

async fn store() -> (FileConfigStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    let store = FileConfigStore::for_workspace(workspace.dir().to_path_buf());
    (store, dir)
}

fn append_unreadable(store: &FileConfigStore, dir: &tempfile::TempDir) {
    let manifest = dir.path().join("ws").join("workspace.toml");
    let text = std::fs::read_to_string(&manifest).expect("read manifest");
    std::fs::write(&manifest, format!("{text}{UNREADABLE}")).expect("write manifest");
    let entries = store.load_saved_searches().expect("load");
    assert!(
        entries
            .iter()
            .any(|entry| matches!(entry, SavedSearchEntry::Unreadable { .. })),
        "fixture must be unreadable"
    );
}

fn unreadable_entries(store: &FileConfigStore) -> Vec<SavedSearchEntry> {
    store
        .load_saved_searches()
        .expect("load")
        .into_iter()
        .filter(|entry| matches!(entry, SavedSearchEntry::Unreadable { .. }))
        .collect()
}

fn people() -> ListQuery {
    ListQuery::new(ListedKind::Person)
}

fn unknown_column() -> ListQuery {
    let mut query = people();
    query.filters.push(ListFilter {
        column: "zodiac".to_owned(),
        op: FilterOp::Contains,
        value: FilterValue::Text("Leo".to_owned()),
    });
    query
}

fn saved_search_error(result: Result<impl std::fmt::Debug, AppError>) -> SavedSearchError {
    match result {
        Err(AppError::SavedSearch(error)) => error,
        other => panic!("expected a saved-search error, got {other:?}"),
    }
}

fn names(store: &FileConfigStore) -> Vec<String> {
    list_saved_searches(store)
        .expect("list")
        .searches
        .into_iter()
        .map(|search| search.name)
        .collect()
}

fn unknown_id() -> SavedSearchId {
    SavedSearchId::from_uuid(Uuid::from_u128(0xDEAD))
}

#[tokio::test]
async fn save_mints_a_uuid_v7_id_and_the_entry_appears_in_list() {
    let (store, _dir) = store().await;

    let id = save_saved_search(&store, &session(), "Tags", &people()).expect("save");

    let parsed: Uuid = id.to_string().parse().expect("id is a uuid");
    assert_eq!(parsed.get_version_num(), 7);
    let list = list_saved_searches(&store).expect("list");
    assert_eq!(list.searches.len(), 1);
    assert_eq!(list.searches[0].id, id);
    assert_eq!(list.searches[0].name, "Tags");
    assert_eq!(list.searches[0].kind, ListedKind::Person);
    assert_eq!(list.unreadable, Vec::new());
}

#[tokio::test]
async fn columns_saved_as_none_stay_distinct_from_columns_never_given() {
    let (store, _dir) = store().await;
    let hidden = ListQuery {
        columns: Some(Vec::new()),
        ..people()
    };
    let unset = ListQuery {
        columns: None,
        ..people()
    };
    save_saved_search(&store, &session(), "Hidden", &hidden).expect("save hidden");
    save_saved_search(&store, &session(), "Unset", &unset).expect("save unset");

    let hidden = resolve_saved_search(&store, "Hidden", None).expect("resolve hidden");
    let unset = resolve_saved_search(&store, "Unset", None).expect("resolve unset");

    assert_eq!(hidden.query.columns, Some(Vec::new()));
    assert_eq!(unset.query.columns, None);
}

#[tokio::test]
async fn update_replaces_the_query_and_keeps_the_name() {
    let (store, _dir) = store().await;
    let id = save_saved_search(&store, &session(), "Mine", &people()).expect("save");

    update_saved_search(&store, id, &ListQuery::new(ListedKind::Place)).expect("update");

    let list = list_saved_searches(&store).expect("list");
    assert_eq!(list.searches.len(), 1);
    assert_eq!(list.searches[0].name, "Mine");
    assert_eq!(list.searches[0].kind, ListedKind::Place);
}

#[tokio::test]
async fn rename_changes_only_the_name() {
    let (store, _dir) = store().await;
    let id = save_saved_search(&store, &session(), "Old", &people()).expect("save");

    rename_saved_search(&store, id, "New").expect("rename");

    let list = list_saved_searches(&store).expect("list");
    assert_eq!(list.searches[0].id, id);
    assert_eq!(list.searches[0].name, "New");
    assert_eq!(list.searches[0].kind, ListedKind::Person);
}

#[tokio::test]
async fn delete_removes_the_entry() {
    let (store, _dir) = store().await;
    let keep = save_saved_search(&store, &session(), "Keep", &people()).expect("save");
    let gone = save_saved_search(&store, &session(), "Gone", &people()).expect("save");

    delete_saved_search(&store, gone).expect("delete");

    let list = list_saved_searches(&store).expect("list");
    assert_eq!(list.searches.len(), 1);
    assert_eq!(list.searches[0].id, keep);
}

#[tokio::test]
async fn names_are_trimmed() {
    let (store, _dir) = store().await;
    let id = save_saved_search(&store, &session(), "  Spaced  ", &people()).expect("save");
    assert_eq!(names(&store), ["Spaced"]);

    rename_saved_search(&store, id, "\tRenamed \n").expect("rename");

    assert_eq!(names(&store), ["Renamed"]);
}

#[tokio::test]
async fn an_empty_or_blank_name_is_refused_on_save_and_rename() {
    let (store, _dir) = store().await;
    let id = save_saved_search(&store, &session(), "Real", &people()).expect("save");

    for blank in ["", "   ", "\t\n"] {
        let on_save = saved_search_error(save_saved_search(&store, &session(), blank, &people()));
        assert!(matches!(on_save, SavedSearchError::EmptyName), "{on_save:?}");
        let on_rename = saved_search_error(rename_saved_search(&store, id, blank));
        assert!(matches!(on_rename, SavedSearchError::EmptyName), "{on_rename:?}");
    }
    assert_eq!(names(&store), ["Real"]);
}

#[tokio::test]
async fn a_duplicate_name_is_refused_ignoring_case_on_save_and_rename() {
    let (store, _dir) = store().await;
    save_saved_search(&store, &session(), "Tags", &people()).expect("save");
    save_saved_search(&store, &session(), "Ærø", &people()).expect("save");
    let other = save_saved_search(&store, &session(), "Other", &people()).expect("save");

    for name in ["tags ", "TAGS", "ærø", " ÆRØ"] {
        let on_save = saved_search_error(save_saved_search(&store, &session(), name, &people()));
        assert!(
            matches!(on_save, SavedSearchError::DuplicateName { .. }),
            "{name}: {on_save:?}"
        );
        let on_rename = saved_search_error(rename_saved_search(&store, other, name));
        assert!(
            matches!(on_rename, SavedSearchError::DuplicateName { .. }),
            "{name}: {on_rename:?}"
        );
    }
    assert_eq!(names(&store), ["Tags", "Ærø", "Other"]);
}

#[tokio::test]
async fn renaming_a_search_to_its_own_name_in_another_case_is_allowed() {
    let (store, _dir) = store().await;
    let id = save_saved_search(&store, &session(), "Ærø", &people()).expect("save");

    rename_saved_search(&store, id, "ærø").expect("rename to own name, other case");

    assert_eq!(names(&store), ["ærø"]);
}

#[tokio::test]
async fn an_unreadable_entrys_name_still_counts_as_taken() {
    let (store, dir) = store().await;
    append_unreadable(&store, &dir);
    let id = save_saved_search(&store, &session(), "Fine", &people()).expect("save");

    let on_save = saved_search_error(save_saved_search(&store, &session(), "BROKEN", &people()));
    assert!(matches!(on_save, SavedSearchError::DuplicateName { .. }), "{on_save:?}");
    let on_rename = saved_search_error(rename_saved_search(&store, id, "broken"));
    assert!(
        matches!(on_rename, SavedSearchError::DuplicateName { .. }),
        "{on_rename:?}"
    );
}

#[tokio::test]
async fn update_rename_and_delete_of_an_unknown_id_are_removed() {
    let (store, _dir) = store().await;
    save_saved_search(&store, &session(), "Present", &people()).expect("save");

    let update = saved_search_error(update_saved_search(&store, unknown_id(), &people()));
    assert!(matches!(update, SavedSearchError::Removed), "{update:?}");
    let rename = saved_search_error(rename_saved_search(&store, unknown_id(), "Other"));
    assert!(matches!(rename, SavedSearchError::Removed), "{rename:?}");
    let delete = saved_search_error(delete_saved_search(&store, unknown_id()));
    assert!(matches!(delete, SavedSearchError::Removed), "{delete:?}");
    assert_eq!(names(&store), ["Present"]);
}

#[tokio::test]
async fn renaming_an_unknown_id_is_removed_even_when_the_name_is_bad() {
    let (store, _dir) = store().await;
    save_saved_search(&store, &session(), "Present", &people()).expect("save");

    for name in ["Present", ""] {
        let rename = saved_search_error(rename_saved_search(&store, unknown_id(), name));
        assert!(matches!(rename, SavedSearchError::Removed), "{name:?}: {rename:?}");
    }
}

#[tokio::test]
async fn a_query_with_an_unknown_column_is_refused_on_save_and_update() {
    let (store, _dir) = store().await;

    let on_save = save_saved_search(&store, &session(), "Zodiac", &unknown_column());
    assert!(
        matches!(on_save, Err(AppError::ListQuery(ListQueryError::UnknownColumn { .. }))),
        "{on_save:?}"
    );
    assert_eq!(names(&store), Vec::<String>::new());

    let id = save_saved_search(&store, &session(), "Fine", &people()).expect("save");
    let on_update = update_saved_search(&store, id, &unknown_column());
    assert!(
        matches!(
            on_update,
            Err(AppError::ListQuery(ListQueryError::UnknownColumn { .. }))
        ),
        "{on_update:?}"
    );
}

#[tokio::test]
async fn list_reports_an_unreadable_entry_with_its_position_and_name() {
    let (store, dir) = store().await;
    save_saved_search(&store, &session(), "First", &people()).expect("save");
    append_unreadable(&store, &dir);

    let list = list_saved_searches(&store).expect("list");

    assert_eq!(list.searches.len(), 1);
    assert_eq!(list.unreadable.len(), 1);
    assert_eq!(list.unreadable[0].position, 1);
    assert_eq!(list.unreadable[0].name.as_deref(), Some("Broken"));
}

#[tokio::test]
async fn save_rename_and_delete_leave_an_unreadable_entry_on_disk_unchanged() {
    let (store, dir) = store().await;
    append_unreadable(&store, &dir);
    let before = unreadable_entries(&store);
    assert_eq!(before.len(), 1);

    let id = save_saved_search(&store, &session(), "Fine", &people()).expect("save");
    assert_eq!(unreadable_entries(&store), before);
    rename_saved_search(&store, id, "Finer").expect("rename");
    assert_eq!(unreadable_entries(&store), before);
    update_saved_search(&store, id, &ListQuery::new(ListedKind::Place)).expect("update");
    assert_eq!(unreadable_entries(&store), before);
    delete_saved_search(&store, id).expect("delete");
    assert_eq!(unreadable_entries(&store), before);

    let manifest: &Path = &dir.path().join("ws").join("workspace.toml");
    let text = std::fs::read_to_string(manifest).expect("read manifest");
    assert!(text.contains("name = \"Broken\""), "{text}");
}

// Resolution and stale references (ADR 0052 §10).

async fn workspace_with_store() -> (Workspace, FileConfigStore, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    let store = FileConfigStore::for_workspace(workspace.dir().to_path_buf());
    (workspace, store, dir)
}

async fn person(ws: &Workspace, given: &str, surname: &str) -> String {
    let new = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some(given.to_owned()),
            Some(surname.to_owned()),
        )),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    create_person(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create person")
}

/// Appends one `[[saved_searches]]` entry straight to `workspace.toml`, as `save` would refuse it.
/// `scalars` are `query` keys (`search`, `columns`); `tables` are sub-tables, written after them.
fn write_search(dir: &tempfile::TempDir, n: u128, name: &str, kind: &str, scalars: &str, tables: &str) {
    let manifest = dir.path().join("ws").join("workspace.toml");
    let text = std::fs::read_to_string(&manifest).expect("read manifest");
    let id = Uuid::from_u128(n);
    let entry = format!(
        "\n[[saved_searches]]\nid = \"{id}\"\nname = \"{name}\"\n[saved_searches.query]\nkind = \"{kind}\"\n\
         {scalars}\n{tables}\n"
    );
    std::fs::write(&manifest, format!("{text}{entry}")).expect("write manifest");
}

fn clause(column: &str, op: &str, value: &str) -> String {
    format!(
        "[[saved_searches.query.filters]]\ncolumn = \"{column}\"\nop = \"{op}\"\n\
         [saved_searches.query.filters.value]\n{value}\n"
    )
}

fn values(items: &[&str]) -> String {
    let quoted: Vec<String> = items.iter().map(|item| format!("\"{item}\"")).collect();
    format!("type = \"values\"\nvalue = [{}]", quoted.join(", "))
}

fn text(value: &str) -> String {
    format!("type = \"text\"\nvalue = \"{value}\"")
}

/// The query a caller may run: none when any clause is stale.
fn runnable(resolution: &SavedSearchResolution) -> Option<&ListQuery> {
    resolution.stale.is_empty().then_some(&resolution.query)
}

async fn listed_persons(ws: &Workspace, resolution: &SavedSearchResolution) -> Vec<String> {
    let query = runnable(resolution).expect("a runnable query");
    list_persons_page(ws, query, ListPage::ALL)
        .await
        .expect("list")
        .items
        .into_iter()
        .map(|summary| summary.human_id)
        .collect()
}

#[tokio::test]
async fn a_missing_tag_narrows_has_any_of_and_widens_has_none_of_never_has_all_of() {
    let (ws, store, dir) = workspace_with_store().await;
    let ola = person(&ws, "Ola", "Olsen").await;
    let per = person(&ws, "Per", "Hansen").await;
    let live = create_tag(&ws, &session(), "Live".to_owned(), Provenance::default(), &[])
        .await
        .expect("tag");
    tag_person(&ws, &session(), &ola, &live, false, MutationMeta::default())
        .await
        .expect("tag ola");
    let gone = Uuid::from_u128(0xDEAD).to_string();
    write_search(
        &dir,
        1,
        "Any",
        "person",
        "",
        &clause("tags", "has-any-of", &values(&[&gone])),
    );
    write_search(
        &dir,
        2,
        "None",
        "person",
        "",
        &clause("tags", "has-none-of", &values(&[&gone])),
    );
    let all_live_and_gone = clause("tags", "has-all-of", &values(&[&live, &gone]));
    write_search(&dir, 3, "All", "person", "", &all_live_and_gone);

    let any = resolve_saved_search(&store, "Any", Some(ListedKind::Person)).expect("resolve any");
    assert_eq!(
        listed_persons(&ws, &any).await,
        Vec::<String>::new(),
        "any of a missing tag"
    );
    let none = resolve_saved_search(&store, "None", Some(ListedKind::Person)).expect("resolve none");
    assert_eq!(listed_persons(&ws, &none).await, [ola, per], "none of a missing tag");
    let all = resolve_saved_search(&store, "All", Some(ListedKind::Person)).expect("resolve all");
    assert_eq!(
        listed_persons(&ws, &all).await,
        Vec::<String>::new(),
        "all of [live, gone] must not fall back to the live tag alone"
    );
    assert_eq!(all.query.filters.len(), 1);
    assert_eq!(
        all.query.filters[0].value,
        FilterValue::Values(vec![live, gone]),
        "stored ids are kept, not pruned"
    );
}

#[tokio::test]
async fn a_missing_record_in_an_is_clause_matches_nothing() {
    let (ws, store, dir) = workspace_with_store().await;
    let new = NewPlace {
        human_id: None,
        place_type: vitni_app::std_statement("farm").unwrap_or_default(),
        name: Some("Nordgard".to_owned()),
    };
    create_place(&ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create place");
    let gone = Uuid::from_u128(0xDEAD).to_string();
    write_search(
        &dir,
        1,
        "Enclosed",
        "place",
        "",
        &clause("enclosed-by", "is", &text(&gone)),
    );

    let resolution = resolve_saved_search(&store, "Enclosed", Some(ListedKind::Place)).expect("resolve");

    assert_eq!(resolution.stale, Vec::new());
    let query = runnable(&resolution).expect("a runnable query");
    let page = list_places_page(&ws, query, ListPage::ALL).await.expect("list");
    assert_eq!(page.total, 0);
}

#[tokio::test]
async fn a_clause_the_catalogue_rejects_is_marked_and_yields_no_runnable_query() {
    let (_ws, store, dir) = workspace_with_store().await;
    let cases = [
        ("Column", clause("nickname", "is-empty", "type = \"none\"")),
        ("Operator", clause("surname", "resembles", &text("Olsen"))),
        ("Unsupported", clause("birth", "contains", &text("17"))),
        ("Choice", clause("sex", "is-any-of", &values(&["femal"]))),
        ("Shape", clause("birth", "before", &text("1800"))),
    ];
    for (n, (name, filters)) in cases.iter().enumerate() {
        write_search(&dir, n as u128 + 1, name, "person", "", filters);
    }

    for (name, _) in &cases {
        let resolution = resolve_saved_search(&store, name, Some(ListedKind::Person)).expect(name);
        assert_eq!(resolution.stale.len(), 1, "{name}: {:?}", resolution.stale);
        assert_eq!(resolution.stale[0].index, 0, "{name}");
        assert!(runnable(&resolution).is_none(), "{name} must not run");
    }
}

#[tokio::test]
async fn a_stale_clause_beside_valid_ones_stops_the_whole_search() {
    let (ws, store, dir) = workspace_with_store().await;
    person(&ws, "Ola", "Olsen").await;
    let filters = format!(
        "{}{}{}",
        clause("surname", "contains", &text("Ols")),
        clause("nickname", "is-empty", "type = \"none\""),
        clause("given", "contains", &text("Ola")),
    );
    write_search(&dir, 1, "Mixed", "person", "", &filters);

    let resolution = resolve_saved_search(&store, "Mixed", Some(ListedKind::Person)).expect("resolve");

    assert_eq!(resolution.stale.len(), 1);
    assert_eq!(resolution.stale[0].index, 1);
    assert_eq!(resolution.stale[0].column.as_deref(), Some("nickname"));
    assert!(runnable(&resolution).is_none(), "no narrower result is produced");
}

#[tokio::test]
async fn a_stale_sort_and_display_column_are_dropped_and_the_rest_runs() {
    let (ws, store, dir) = workspace_with_store().await;
    let ola = person(&ws, "Ola", "Olsen").await;
    person(&ws, "Per", "Hansen").await;
    write_search(
        &dir,
        1,
        "Drift",
        "person",
        "columns = [\"surname\", \"zodiac\"]",
        &format!(
            "{}[saved_searches.query.sort]\ncolumn = \"zodiac\"\ndirection = \"ascending\"\n",
            clause("surname", "contains", &text("ols"))
        ),
    );

    let resolution = resolve_saved_search(&store, "Drift", Some(ListedKind::Person)).expect("resolve");

    assert_eq!(resolution.stale, Vec::new());
    assert_eq!(resolution.query.sort, None);
    assert_eq!(resolution.query.columns, Some(vec!["surname".to_owned()]));
    assert_eq!(listed_persons(&ws, &resolution).await, [ola]);
}

#[tokio::test]
async fn a_stored_sort_on_the_tags_column_is_dropped_and_the_rest_runs() {
    let (ws, store, dir) = workspace_with_store().await;
    let ola = person(&ws, "Ola", "Olsen").await;
    person(&ws, "Per", "Hansen").await;
    write_search(
        &dir,
        1,
        "TagSort",
        "person",
        "",
        &format!(
            "{}[saved_searches.query.sort]\ncolumn = \"tags\"\ndirection = \"ascending\"\n",
            clause("surname", "contains", &text("ols"))
        ),
    );

    let resolution = resolve_saved_search(&store, "TagSort", Some(ListedKind::Person)).expect("resolve");

    assert_eq!(resolution.stale, Vec::new());
    assert_eq!(resolution.query.sort, None);
    assert_eq!(listed_persons(&ws, &resolution).await, [ola]);
}

#[tokio::test]
async fn a_stored_sort_carrying_a_key_this_version_does_not_model_is_dropped() {
    let (ws, store, dir) = workspace_with_store().await;
    let ola = person(&ws, "Ola", "Olsen").await;
    person(&ws, "Per", "Hansen").await;
    write_search(
        &dir,
        1,
        "NullsSort",
        "person",
        "",
        &format!(
            "{}[saved_searches.query.sort]\ncolumn = \"surname\"\ndirection = \"descending\"\nnulls = \"last\"\n",
            clause("surname", "contains", &text("ols"))
        ),
    );

    let resolution = resolve_saved_search(&store, "NullsSort", Some(ListedKind::Person)).expect("resolve");

    assert_eq!(resolution.stale, Vec::new());
    assert_eq!(resolution.query.sort, None);
    assert_eq!(listed_persons(&ws, &resolution).await, [ola]);
}

#[tokio::test]
async fn resolving_for_another_kind_names_both_kinds() {
    let (_ws, store, _dir) = workspace_with_store().await;
    save_saved_search(&store, &session(), "Tags", &people()).expect("save");

    let result = resolve_saved_search(&store, "Tags", Some(ListedKind::Place));

    assert!(
        matches!(
            result,
            Err(AppError::ListQuery(ListQueryError::KindMismatch {
                expected: ListedKind::Place,
                found: ListedKind::Person,
            }))
        ),
        "{result:?}"
    );
}

#[tokio::test]
async fn an_unknown_name_is_not_found() {
    let (_ws, store, _dir) = workspace_with_store().await;
    save_saved_search(&store, &session(), "Tags", &people()).expect("save");

    let error = saved_search_error(resolve_saved_search(&store, "Nope", None));

    assert!(matches!(error, SavedSearchError::NotFound { .. }), "{error:?}");
}

#[tokio::test]
async fn lookup_ignores_case_and_also_accepts_the_id() {
    let (_ws, store, _dir) = workspace_with_store().await;
    let id = save_saved_search(&store, &session(), "Ærø Tags", &people()).expect("save");

    let by_name = resolve_saved_search(&store, "ærø TAGS", Some(ListedKind::Person)).expect("by name");
    let by_id = resolve_saved_search(&store, &id.to_string(), None).expect("by id");

    assert_eq!(by_name.search.id, id);
    assert_eq!(by_id.search.id, id);
    assert_eq!(by_id.search.name, "Ærø Tags");
}

#[tokio::test]
async fn lookup_by_id_ignores_the_case_of_the_id() {
    let (_ws, store, _dir) = workspace_with_store().await;
    let id = save_saved_search(&store, &session(), "Shouted", &people()).expect("save");

    let found =
        resolve_saved_search(&store, &format!("  {}  ", id.to_string().to_uppercase()), None).expect("by uppercase id");

    assert_eq!(found.search.id, id);
}

#[tokio::test]
async fn typed_id_lookup_finds_the_search_and_checks_the_kind() {
    let (_ws, store, _dir) = workspace_with_store().await;
    let id = save_saved_search(&store, &session(), "Typed", &people()).expect("save");

    let found = resolve_saved_search_id(&store, id, Some(ListedKind::Person)).expect("by id");
    let wrong_kind = resolve_saved_search_id(&store, id, Some(ListedKind::Place));

    assert_eq!(found.search.name, "Typed");
    assert!(wrong_kind.is_err());
}

#[tokio::test]
async fn a_stale_clause_carries_the_reason_the_catalogue_gave() {
    let (_ws, store, dir) = workspace_with_store().await;
    let cases = [
        (
            "Column",
            clause("nickname", "is-empty", "type = \"none\""),
            ListQueryError::UnknownColumn {
                kind: ListedKind::Person,
                column: "nickname".to_owned(),
            },
        ),
        (
            "Operator",
            clause("surname", "resembles", &text("Olsen")),
            ListQueryError::UnknownOperator {
                column: "surname".to_owned(),
                op: "resembles".to_owned(),
            },
        ),
        (
            "Value",
            clause("surname", "contains", "type = \"hologram\""),
            ListQueryError::WrongValue {
                column: "surname".to_owned(),
                op: FilterOp::Contains,
            },
        ),
    ];
    for (n, (name, filters, _)) in cases.iter().enumerate() {
        write_search(&dir, n as u128 + 1, name, "person", "", filters);
    }

    for (name, _, expected) in cases {
        let resolution = resolve_saved_search(&store, name, Some(ListedKind::Person)).expect(name);
        assert_eq!(resolution.stale[0].error, expected, "{name}");
    }
}

#[tokio::test]
async fn a_typed_id_lookup_of_a_deleted_search_is_removed_not_named_by_its_id() {
    let (_ws, store, _dir) = workspace_with_store().await;

    let error = saved_search_error(resolve_saved_search_id(&store, unknown_id(), None));

    assert!(matches!(error, SavedSearchError::Removed), "{error:?}");
}

#[tokio::test]
async fn a_clause_with_a_key_this_version_does_not_model_is_marked_not_run() {
    let (_ws, store, dir) = workspace_with_store().await;
    let filters = "[[saved_searches.query.filters]]\ncolumn = \"surname\"\nop = \"contains\"\nnegate = true\n\
                   [saved_searches.query.filters.value]\ntype = \"text\"\nvalue = \"Olsen\"\n";
    write_search(&dir, 1, "Newer", "person", "", filters);

    let resolution = resolve_saved_search(&store, "Newer", Some(ListedKind::Person)).expect("resolve");

    assert_eq!(resolution.stale.len(), 1, "{:?}", resolution.stale);
    assert!(runnable(&resolution).is_none(), "an unmodelled key must not be ignored");
}

#[tokio::test]
async fn a_clause_missing_its_column_or_operator_reports_none_not_an_empty_name() {
    let (_ws, store, dir) = workspace_with_store().await;
    let no_column = format!(
        "[[saved_searches.query.filters]]\nop = \"contains\"\n[saved_searches.query.filters.value]\n{}\n",
        text("x")
    );
    let no_op = format!(
        "[[saved_searches.query.filters]]\ncolumn = \"surname\"\n[saved_searches.query.filters.value]\n{}\n",
        text("x")
    );
    write_search(&dir, 1, "NoColumn", "person", "", &no_column);
    write_search(&dir, 2, "NoOp", "person", "", &no_op);

    let first = resolve_saved_search(&store, "NoColumn", Some(ListedKind::Person)).expect("resolve");
    let second = resolve_saved_search(&store, "NoOp", Some(ListedKind::Person)).expect("resolve");

    assert_eq!(first.stale[0].column, None);
    assert_eq!(first.stale[0].op.as_deref(), Some("contains"));
    assert_eq!(second.stale[0].column.as_deref(), Some("surname"));
    assert_eq!(second.stale[0].op, None);
}

#[tokio::test]
async fn a_saved_place_type_choice_clause_is_stale_now_that_type_is_text() {
    let (_ws, store, dir) = workspace_with_store().await;
    write_search(
        &dir,
        1,
        "Parishes",
        "place",
        "",
        &clause("type", "is-any-of", &values(&["parish"])),
    );

    let resolution = resolve_saved_search(&store, "Parishes", Some(ListedKind::Place)).expect("resolve");

    assert_eq!(resolution.stale.len(), 1, "{:?}", resolution.stale);
    assert_eq!(resolution.stale[0].column.as_deref(), Some("type"));
    assert_eq!(resolution.stale[0].op.as_deref(), Some("is-any-of"));
    let error = saved_search_error(vitni_app::runnable_saved_query(&store, "Parishes", ListedKind::Place));
    assert!(matches!(error, SavedSearchError::Broken { .. }), "{error:?}");
}

#[tokio::test]
async fn a_saved_search_naming_place_kind_or_level_runs() {
    let (ws, store, dir) = workspace_with_store().await;
    let new = |name: &str, kind, level| NewPlace {
        human_id: None,
        place_type: vitni_app::PlaceTypeStatement {
            kind,
            level,
            type_key: None,
            date: None,
        },
        name: Some(name.to_owned()),
    };
    let hamar = create_place(
        &ws,
        &session(),
        new(
            "Hamar",
            Some(vitni_app::PlaceKind::Ecclesiastical),
            Some(vitni_app::PlaceLevel::Adm1),
        ),
        Provenance::default(),
        &[],
    )
    .await
    .expect("hamar");
    create_place(
        &ws,
        &session(),
        new(
            "Norge",
            Some(vitni_app::PlaceKind::Administrative),
            Some(vitni_app::PlaceLevel::Country),
        ),
        Provenance::default(),
        &[],
    )
    .await
    .expect("norge");
    let filters = format!(
        "{}{}",
        clause("kind", "is-any-of", &values(&["ecclesiastical"])),
        clause("level", "is-none-of", &values(&["country"]))
    );
    write_search(&dir, 1, "Church levels", "place", "", &filters);

    let resolution = resolve_saved_search(&store, "Church levels", Some(ListedKind::Place)).expect("resolve");

    assert_eq!(resolution.stale, Vec::new());
    let query = runnable(&resolution).expect("a runnable query");
    let page = list_places_page(&ws, query, ListPage::ALL).await.expect("list");
    let found: Vec<String> = page.items.into_iter().map(|summary| summary.human_id).collect();
    assert_eq!(found, [hamar]);
}
