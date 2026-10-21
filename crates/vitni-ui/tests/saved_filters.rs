//! Rebuilding the filter chips of a saved search (ADR 0052 §3, §10): a tag or record held by id shows
//! by name, a missing one shows as deleted without its id, and a clause the catalogue cannot take is a
//! marked clause, never an applied filter.
//!
//! Runs against a real on-disk workspace over `vitni-app`'s public surface only (ADR 0008).

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    Agent, AgentId, AgentKind, AppDefaults, ConfigStore, DateParts, FileConfigStore, FilterOp, FilterValue, ListFilter,
    ListQuery, ListedKind, NewPlace, OperatorConfig, PlaceType, Provenance, SavedSearchResolution, Session, Workspace,
    WorkspaceDefaults, create_place, create_tag, gregorian_date, reference_id, resolve_saved_search, save_saved_search,
};
use vitni_ui::{ActiveFilter, Localizer, filter_chip_label, resolve_filter, saved_filters};

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

struct Fixture {
    workspace: Workspace,
    store: FileConfigStore,
    loc: Localizer,
    dir: tempfile::TempDir,
}

async fn setup() -> Fixture {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    let store = FileConfigStore::for_workspace(workspace.dir().to_path_buf());
    let loc = Localizer::for_workspace(&ws, None);
    Fixture {
        workspace,
        store,
        loc,
        dir,
    }
}

fn year(year: i32) -> vitni_app::GenealogicalDate {
    gregorian_date(DateParts {
        year,
        month: None,
        day: None,
    })
}

/// Saves `filters` over `kind` and resolves the saved search again, as applying it does.
fn saved(fixture: &Fixture, kind: ListedKind, filters: Vec<ListFilter>) -> SavedSearchResolution {
    let query = ListQuery {
        filters,
        ..ListQuery::new(kind)
    };
    save_saved_search(&fixture.store, &session(), "Mine", &query).expect("save");
    resolve_saved_search(&fixture.store, "Mine", Some(kind)).expect("resolve")
}

fn tag_clause(op: FilterOp, ids: &[&str]) -> ListFilter {
    ListFilter {
        column: "tags".to_owned(),
        op,
        value: FilterValue::Values(ids.iter().map(|id| (*id).to_owned()).collect()),
    }
}

async fn place(fixture: &Fixture, name: &str) -> String {
    let new = NewPlace {
        human_id: None,
        place_type: PlaceType::Parish,
        name: Some(name.to_owned()),
    };
    create_place(&fixture.workspace, &session(), new, Provenance::default(), &[])
        .await
        .expect("place")
}

fn place_clause(id: &str) -> ListFilter {
    ListFilter {
        column: "place".to_owned(),
        op: FilterOp::Is,
        value: FilterValue::Text(id.to_owned()),
    }
}

fn chip(fixture: &Fixture, kind: ListedKind, active: &ActiveFilter) -> String {
    filter_chip_label(&fixture.loc, kind, active)
}

#[tokio::test]
async fn a_saved_tag_clause_shows_the_tag_name() {
    let fixture = setup().await;
    let tag = create_tag(
        &fixture.workspace,
        &session(),
        "To verify".to_owned(),
        Provenance::default(),
        &[],
    )
    .await
    .expect("tag");
    let resolution = saved(
        &fixture,
        ListedKind::Person,
        vec![tag_clause(FilterOp::HasAllOf, &[&tag])],
    );

    let filters = saved_filters(&fixture.workspace, &fixture.loc, &resolution)
        .await
        .expect("filters");

    assert_eq!(filters.stale.len(), 0);
    assert_eq!(filters.applied.len(), 1);
    let text = chip(&fixture, ListedKind::Person, &filters.applied[0]);
    assert!(text.contains("To verify"), "{text}");
    assert!(!text.contains(&tag), "the id never shows: {text}");
    assert_eq!(filters.applied[0].filter, resolution.query.filters[0]);
}

#[tokio::test]
async fn a_saved_clause_on_a_deleted_tag_says_so_and_keeps_the_id_in_the_query() {
    let fixture = setup().await;
    let gone = Uuid::from_u128(0xdead).to_string();
    let resolution = saved(
        &fixture,
        ListedKind::Person,
        vec![tag_clause(FilterOp::HasAnyOf, &[&gone])],
    );

    let filters = saved_filters(&fixture.workspace, &fixture.loc, &resolution)
        .await
        .expect("filters");

    let text = chip(&fixture, ListedKind::Person, &filters.applied[0]);
    assert!(text.to_lowercase().contains("deleted tag"), "{text}");
    assert!(!text.contains(&gone), "no UUID in the chip: {text}");
    assert_eq!(
        filters.applied[0].filter.value,
        FilterValue::Values(vec![gone]),
        "the stored id stays, so the clause still matches nothing"
    );
}

#[tokio::test]
async fn a_saved_record_clause_reopens_on_the_records_human_id() {
    let fixture = setup().await;
    let human_id = place(&fixture, "Fana").await;
    let id = reference_id(&fixture.workspace, ListedKind::Event, "place", &human_id)
        .await
        .expect("id");
    let resolution = saved(&fixture, ListedKind::Event, vec![place_clause(&id)]);

    let filters = saved_filters(&fixture.workspace, &fixture.loc, &resolution)
        .await
        .expect("filters");

    let active = &filters.applied[0];
    assert_eq!(active.draft.values, std::slice::from_ref(&human_id));
    let text = chip(&fixture, ListedKind::Event, active);
    assert!(text.contains("Fana") && !text.contains(&id), "{text}");
    assert_eq!(
        active.draft.labels.get(&human_id).map(String::as_str),
        Some("Fana"),
        "held by the bare title, as a fresh pick is, so the picker does not show the id twice"
    );
    let reapplied = active.draft.apply(ListedKind::Event).expect("apply");
    let back = resolve_filter(&fixture.workspace, ListedKind::Event, reapplied)
        .await
        .expect("resolve");
    assert_eq!(
        back.filter, resolution.query.filters[0],
        "converts back to the same filter"
    );
}

#[tokio::test]
async fn a_saved_clause_on_a_deleted_record_says_so_without_its_id() {
    let fixture = setup().await;
    let gone = Uuid::from_u128(0xbeef).to_string();
    let resolution = saved(&fixture, ListedKind::Event, vec![place_clause(&gone)]);

    let filters = saved_filters(&fixture.workspace, &fixture.loc, &resolution)
        .await
        .expect("filters");

    let text = chip(&fixture, ListedKind::Event, &filters.applied[0]);
    assert!(text.to_lowercase().contains("deleted record"), "{text}");
    assert!(!text.contains(&gone), "{text}");
    let back = resolve_filter(
        &fixture.workspace,
        ListedKind::Event,
        filters.applied[0].draft.apply(ListedKind::Event).expect("apply"),
    )
    .await
    .expect("resolve");
    assert_eq!(back.filter, resolution.query.filters[0]);
}

#[tokio::test]
async fn date_and_number_ranges_reopen_with_both_inputs_and_read_back_the_same() {
    let fixture = setup().await;
    let dates = ListFilter {
        column: "birth".to_owned(),
        op: FilterOp::Between,
        value: FilterValue::DateRange {
            from: year(1750),
            to: year(1800),
        },
    };
    let numbers = ListFilter {
        column: "shared-cm".to_owned(),
        op: FilterOp::Between,
        value: FilterValue::NumberRange { from: 7.5, to: 20.0 },
    };
    let person = saved(&fixture, ListedKind::Person, vec![dates]);
    let dna = {
        let query = ListQuery {
            filters: vec![numbers],
            ..ListQuery::new(ListedKind::DnaMatch)
        };
        save_saved_search(&fixture.store, &session(), "Matches", &query).expect("save");
        resolve_saved_search(&fixture.store, "Matches", None).expect("resolve")
    };

    for (kind, resolution) in [(ListedKind::Person, person), (ListedKind::DnaMatch, dna)] {
        let filters = saved_filters(&fixture.workspace, &fixture.loc, &resolution)
            .await
            .expect("filters");
        let draft = &filters.applied[0].draft;
        assert!(
            !draft.first.is_empty() && !draft.second.is_empty(),
            "{kind:?}: {draft:?}"
        );
        let back = draft.apply(kind).expect("the inputs read back");
        assert_eq!(back.filter, resolution.query.filters[0], "{kind:?}");
    }
}

#[tokio::test]
async fn a_saved_choice_clause_keeps_its_keys() {
    let fixture = setup().await;
    let clause = ListFilter {
        column: "sex".to_owned(),
        op: FilterOp::IsAnyOf,
        value: FilterValue::Values(vec!["female".to_owned()]),
    };
    let resolution = saved(&fixture, ListedKind::Person, vec![clause]);

    let filters = saved_filters(&fixture.workspace, &fixture.loc, &resolution)
        .await
        .expect("filters");

    assert_eq!(filters.applied[0].draft.values, ["female"]);
    assert_eq!(filters.applied[0].filter, resolution.query.filters[0]);
}

#[tokio::test]
async fn a_stale_clause_is_a_marked_clause_never_an_applied_filter() {
    let fixture = setup().await;
    let manifest = fixture.dir.path().join("ws").join("workspace.toml");
    let text = std::fs::read_to_string(&manifest).expect("read manifest");
    let entry = format!(
        "\n[[saved_searches]]\nid = \"{}\"\nname = \"Stale\"\n[saved_searches.query]\nkind = \"person\"\n\
         [[saved_searches.query.filters]]\ncolumn = \"zodiac\"\nop = \"contains\"\n\
         [saved_searches.query.filters.value]\ntype = \"text\"\nvalue = \"Leo\"\n",
        Uuid::from_u128(9)
    );
    std::fs::write(&manifest, format!("{text}{entry}")).expect("write manifest");
    assert!(fixture.store.load_saved_searches().is_ok());
    let resolution = resolve_saved_search(&fixture.store, "Stale", None).expect("resolve");
    assert_eq!(resolution.stale.len(), 1, "fixture must be stale");

    let filters = saved_filters(&fixture.workspace, &fixture.loc, &resolution)
        .await
        .expect("filters");

    assert!(filters.applied.is_empty(), "{:?}", filters.applied);
    assert_eq!(filters.stale.len(), 1);
    assert!(
        filters.stale[0].label.contains("zodiac contains"),
        "{:?}",
        filters.stale[0]
    );
}

#[tokio::test]
async fn a_search_with_no_clauses_has_no_chips() {
    let fixture = setup().await;
    let resolution = saved(&fixture, ListedKind::Person, Vec::new());
    let filters = saved_filters(&fixture.workspace, &fixture.loc, &resolution)
        .await
        .expect("filters");
    assert!(filters.applied.is_empty() && filters.stale.is_empty());
}

async fn stale_label_for(filter_keys: &str) -> String {
    let fixture = setup().await;
    let manifest = fixture.dir.path().join("ws").join("workspace.toml");
    let text = std::fs::read_to_string(&manifest).expect("read manifest");
    let entry = format!(
        "\n[[saved_searches]]\nid = \"{}\"\nname = \"Gap\"\n[saved_searches.query]\nkind = \"person\"\n\
         [[saved_searches.query.filters]]\n{filter_keys}\n\
         [saved_searches.query.filters.value]\ntype = \"text\"\nvalue = \"Leo\"\n",
        Uuid::from_u128(9)
    );
    std::fs::write(&manifest, format!("{text}{entry}")).expect("write manifest");
    let resolution = resolve_saved_search(&fixture.store, "Gap", None).expect("resolve");
    assert_eq!(resolution.stale.len(), 1, "fixture must be stale");
    let filters = saved_filters(&fixture.workspace, &fixture.loc, &resolution)
        .await
        .expect("filters");
    assert!(filters.applied.is_empty(), "{:?}", filters.applied);
    filters.stale[0].label.clone()
}

#[tokio::test]
async fn a_stale_clause_without_a_column_says_it_names_none() {
    let label = stale_label_for("op = \"contains\"").await;
    assert_eq!(label, "contains (this clause names no column)");
}

#[tokio::test]
async fn a_stale_clause_without_an_operator_says_it_names_none() {
    let label = stale_label_for("column = \"surname\"").await;
    assert_eq!(label, "surname (this clause names no operator)");
}
