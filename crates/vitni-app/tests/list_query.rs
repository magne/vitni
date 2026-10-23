//! List queries (ADR 0052 §1–4, §10): a typed, serde `ListQuery` validated against the kind's column
//! catalogue and evaluated by the store, and the paged list use-cases that take one.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AppDefaults, AppError, DateParts, FilterOp, FilterValue, ListCell, ListFilter, ListPage, ListQuery, ListQueryError,
    ListSort, ListWindow, ListedKind, MutationMeta, NewPerson, OperatorConfig, PersonNameParts, PlaceKind, PlaceLevel,
    Provenance, Session, SortDirection, Workspace, WorkspaceDefaults, assert_sex, create_person, create_tag,
    gregorian_date, list_persons_page, list_places_page, list_tags_query, query_list, record_birth, tag_person,
};
use vitni_core::date::GenealogicalDate;
use vitni_core::enums::{EvidenceLevel, Sex};
use vitni_core::ids::AgentId;
use vitni_core::provenance::{Agent, AgentKind};

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

async fn workspace() -> (Workspace, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    (workspace, dir)
}

async fn person(ws: &Workspace, given: &str, surname: &str, born: Option<i32>) -> String {
    let new = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some(given.to_owned()),
            Some(surname.to_owned()),
        )),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    let human_id = create_person(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create person");
    if let Some(year) = born {
        let date = DateParts {
            year,
            month: None,
            day: None,
        };
        record_birth(ws, &session(), &human_id, date, MutationMeta::default())
            .await
            .expect("record birth");
    }
    human_id
}

fn date(year: i32) -> GenealogicalDate {
    gregorian_date(DateParts {
        year,
        month: None,
        day: None,
    })
}

fn year(year: i32) -> FilterValue {
    FilterValue::Date(date(year))
}

fn filter(column: &str, op: FilterOp, value: FilterValue) -> ListFilter {
    ListFilter {
        column: column.to_owned(),
        op,
        value,
    }
}

fn persons(filters: Vec<ListFilter>) -> ListQuery {
    ListQuery {
        filters,
        ..ListQuery::new(ListedKind::Person)
    }
}

const fn page(offset: u32, limit: u32) -> ListPage {
    ListPage {
        offset,
        limit: Some(limit),
    }
}

/// The `human_id`s a query lists, in order.
async fn listed(ws: &Workspace, query: &ListQuery) -> Vec<String> {
    list_persons_page(ws, query, ListPage::ALL)
        .await
        .expect("list")
        .items
        .into_iter()
        .map(|summary| summary.human_id)
        .collect()
}

#[tokio::test]
async fn a_person_page_lists_the_query_in_its_order_with_the_total() {
    let (ws, _dir) = workspace().await;
    let ola = person(&ws, "Ola", "Olsen", Some(1790)).await;
    let kari = person(&ws, "Kari", "Olsen", Some(1810)).await;
    person(&ws, "Per", "Hansen", None).await;
    let query = ListQuery {
        sort: Some(ListSort {
            column: "birth".to_owned(),
            direction: SortDirection::Descending,
        }),
        ..persons(vec![filter(
            "surname",
            FilterOp::Contains,
            FilterValue::Text("OLS".to_owned()),
        )])
    };

    let first = list_persons_page(&ws, &query, page(0, 1)).await.expect("first page");
    assert_eq!(first.total, 2);
    let ids: Vec<&str> = first.items.iter().map(|summary| summary.human_id.as_str()).collect();
    assert_eq!(ids, [kari.as_str()], "the latest birth first");
    assert_eq!(listed(&ws, &query).await, [kari, ola]);
}

#[tokio::test]
async fn a_date_filter_compares_whole_periods() {
    let (ws, _dir) = workspace().await;
    let ola = person(&ws, "Ola", "Olsen", Some(1790)).await;
    let kari = person(&ws, "Kari", "Olsen", Some(1810)).await;

    let before = persons(vec![filter("birth", FilterOp::Before, year(1800))]);
    assert_eq!(listed(&ws, &before).await, std::slice::from_ref(&ola));
    let after = persons(vec![filter("birth", FilterOp::After, year(1790))]);
    assert_eq!(listed(&ws, &after).await, [kari], "after 1790 is after all of 1790");
    let within = FilterValue::DateRange {
        from: date(1790),
        to: date(1790),
    };
    let between = persons(vec![filter("birth", FilterOp::Between, within)]);
    assert_eq!(listed(&ws, &between).await, [ola]);
}

#[tokio::test]
async fn a_choice_filter_takes_only_the_columns_values() {
    let (ws, _dir) = workspace().await;
    let kari = person(&ws, "Kari", "Olsen", None).await;
    person(&ws, "Per", "Hansen", None).await;
    assert_sex(&ws, &session(), &kari, Sex::Female, MutationMeta::default())
        .await
        .expect("sex");

    let women = persons(vec![filter(
        "sex",
        FilterOp::IsAnyOf,
        FilterValue::Values(vec!["Female".to_owned()]),
    )]);
    assert_eq!(listed(&ws, &women).await, [kari]);

    let typo = persons(vec![filter(
        "sex",
        FilterOp::IsNoneOf,
        FilterValue::Values(vec!["femal".to_owned()]),
    )]);
    let refused = list_persons_page(&ws, &typo, ListPage::ALL).await;
    assert!(
        matches!(
            refused,
            Err(AppError::ListQuery(ListQueryError::UnknownChoice { ref value, .. })) if value == "femal"
        ),
        "a misspelt value is refused, not matched as nothing: {refused:?}"
    );
}

#[tokio::test]
async fn a_tag_that_does_not_exist_narrows_the_result_never_widens_it() {
    let (ws, _dir) = workspace().await;
    let ola = person(&ws, "Ola", "Olsen", None).await;
    let per = person(&ws, "Per", "Hansen", None).await;
    let tag = create_tag(&ws, &session(), "To verify".to_owned(), Provenance::default(), &[])
        .await
        .expect("tag");
    tag_person(&ws, &session(), &ola, &tag, false, MutationMeta::default())
        .await
        .expect("tag ola");
    let gone = Uuid::from_u128(0xDEAD).to_string();
    let tags = |op, ids: &[&str]| {
        persons(vec![filter(
            "tags",
            op,
            FilterValue::Values(ids.iter().map(|id| (*id).to_owned()).collect()),
        )])
    };

    assert_eq!(
        listed(&ws, &tags(FilterOp::HasAnyOf, &[&tag])).await,
        std::slice::from_ref(&ola)
    );
    assert!(
        listed(&ws, &tags(FilterOp::HasAnyOf, &[&gone])).await.is_empty(),
        "any of a missing tag finds no one"
    );
    assert!(
        listed(&ws, &tags(FilterOp::HasAllOf, &[&tag, &gone])).await.is_empty(),
        "all of a missing tag finds no one"
    );
    assert_eq!(
        listed(&ws, &tags(FilterOp::HasNoneOf, &[&gone])).await,
        [ola, per],
        "none of a missing tag finds everyone"
    );
}

#[tokio::test]
async fn a_query_the_catalogue_cannot_evaluate_is_refused_naming_the_clause() {
    let (ws, _dir) = workspace().await;
    person(&ws, "Ola", "Olsen", Some(1790)).await;
    let refusals = [
        (
            persons(vec![filter("nickname", FilterOp::IsEmpty, FilterValue::None)]),
            ListQueryError::UnknownColumn {
                kind: ListedKind::Person,
                column: "nickname".to_owned(),
            },
        ),
        (
            persons(vec![filter(
                "birth",
                FilterOp::Contains,
                FilterValue::Text("17".to_owned()),
            )]),
            ListQueryError::UnsupportedOperator {
                column: "birth".to_owned(),
                op: FilterOp::Contains,
            },
        ),
        (
            persons(vec![filter(
                "birth",
                FilterOp::Before,
                FilterValue::Text("1800".to_owned()),
            )]),
            ListQueryError::WrongValue {
                column: "birth".to_owned(),
                op: FilterOp::Before,
            },
        ),
        (
            ListQuery {
                sort: Some(ListSort {
                    column: "tags".to_owned(),
                    direction: SortDirection::Ascending,
                }),
                ..ListQuery::new(ListedKind::Person)
            },
            ListQueryError::Unsortable {
                column: "tags".to_owned(),
            },
        ),
    ];
    for (query, expected) in refusals {
        match list_persons_page(&ws, &query, ListPage::ALL).await {
            Err(AppError::ListQuery(error)) => assert_eq!(error, expected),
            other => panic!("{query:?} gave {other:?}"),
        }
    }
    let places = ListQuery::new(ListedKind::Place);
    let mismatch = list_persons_page(&ws, &places, ListPage::ALL).await;
    assert!(
        matches!(
            mismatch,
            Err(AppError::ListQuery(ListQueryError::KindMismatch {
                expected: ListedKind::Person,
                found: ListedKind::Place
            }))
        ),
        "{mismatch:?}"
    );
    assert!(list_places_page(&ws, &places, ListPage::ALL).await.is_ok());
}

#[tokio::test]
async fn a_query_returns_typed_cells_for_its_columns() {
    let (ws, _dir) = workspace().await;
    person(&ws, "Ola", "Olsen", Some(1790)).await;
    let query = ListQuery {
        columns: Some(vec!["name".to_owned(), "birth".to_owned()]),
        ..ListQuery::new(ListedKind::Person)
    };
    let window = ListWindow::Offset { offset: 0, limit: None };
    let result = query_list(&ws, &query, window).await.expect("query");
    assert_eq!(result.total, Some(1));
    assert_eq!(result.rows[0].cells[0], Some(ListCell::Text("Ola Olsen".to_owned())));
    assert!(
        matches!(&result.rows[0].cells[1], Some(ListCell::Date(date)) if date.sort_value == 17_900_000),
        "{:?}",
        result.rows[0].cells
    );
}

#[tokio::test]
async fn a_page_carries_each_rows_cells_under_its_human_id() {
    let (ws, _dir) = workspace().await;
    let ola = person(&ws, "Ola", "Olsen", Some(1790)).await;
    let per = person(&ws, "Per", "Hansen", None).await;
    let query = ListQuery {
        columns: Some(vec!["surname".to_owned(), "birth".to_owned()]),
        ..ListQuery::new(ListedKind::Person)
    };

    let page = list_persons_page(&ws, &query, ListPage::ALL).await.expect("page");

    let ola_cells = page.cells.get(&ola).expect("Ola's cells");
    assert_eq!(ola_cells[0], Some(ListCell::Text("Olsen".to_owned())));
    assert!(matches!(&ola_cells[1], Some(ListCell::Date(_))), "{ola_cells:?}");
    assert_eq!(page.cells.get(&per).expect("Per's cells")[1], None, "no birth, no cell");
    let bare = list_persons_page(&ws, &ListQuery::new(ListedKind::Person), ListPage::ALL)
        .await
        .expect("bare page");
    assert_eq!(
        bare.cells.get(&ola),
        Some(&Vec::new()),
        "no columns asked, none returned"
    );
}

#[tokio::test]
async fn a_tag_page_carries_its_cells_under_the_tag_id() {
    let (ws, _dir) = workspace().await;
    create_tag(&ws, &session(), "Verified".to_owned(), Provenance::default(), &[])
        .await
        .expect("tag");
    let query = ListQuery {
        columns: Some(vec!["name".to_owned()]),
        ..ListQuery::new(ListedKind::Tag)
    };

    let page = list_tags_query(&ws, &query).await.expect("tags");

    assert_eq!(page.total, 1);
    let tag = &page.items[0];
    assert_eq!(
        page.cells.get(&tag.id),
        Some(&vec![Some(ListCell::Text("Verified".to_owned()))])
    );
}

#[test]
fn a_query_serializes_by_column_and_operator_name() {
    let query = ListQuery {
        search: "ola".to_owned(),
        sort: Some(ListSort {
            column: "birth".to_owned(),
            direction: SortDirection::Descending,
        }),
        columns: Some(vec!["birth".to_owned()]),
        ..persons(vec![filter(
            "sex",
            FilterOp::IsAnyOf,
            FilterValue::Values(vec!["female".to_owned()]),
        )])
    };
    let json = serde_json::to_value(&query).expect("serialize");
    assert_eq!(
        json,
        serde_json::json!({
            "kind": "person",
            "search": "ola",
            "filters": [{ "column": "sex", "op": "is-any-of", "value": { "type": "values", "value": ["female"] } }],
            "sort": { "column": "birth", "direction": "descending" },
            "columns": ["birth"],
        })
    );
    assert_eq!(serde_json::from_value::<ListQuery>(json).expect("deserialize"), query);
    let bare: ListQuery = serde_json::from_value(serde_json::json!({ "kind": "dna-test" })).expect("defaults");
    assert_eq!(bare, ListQuery::new(ListedKind::DnaTest));
}

#[tokio::test]
async fn a_tag_query_runs_over_the_whole_vocabulary_in_its_order() {
    let (ws, _dir) = workspace().await;
    for name in ["Verified", "To verify", "Unsure"] {
        create_tag(&ws, &session(), name.to_owned(), Provenance::default(), &[])
            .await
            .expect("tag");
    }
    let query = ListQuery {
        search: "VER".to_owned(),
        sort: Some(ListSort {
            column: "name".to_owned(),
            direction: SortDirection::Descending,
        }),
        ..ListQuery::new(ListedKind::Tag)
    };

    let names: Vec<_> = list_tags_query(&ws, &query)
        .await
        .expect("tags")
        .items
        .into_iter()
        .map(|tag| tag.name.unwrap_or_default())
        .collect();

    assert_eq!(names, ["Verified", "To verify"]);
    assert!(matches!(
        list_tags_query(&ws, &persons(Vec::new())).await,
        Err(AppError::ListQuery(ListQueryError::KindMismatch { .. }))
    ));
}

async fn classified_place(ws: &Workspace, name: &str, kind: Option<PlaceKind>, level: Option<PlaceLevel>) -> String {
    let new = vitni_app::NewPlace {
        human_id: None,
        place_type: vitni_app::PlaceTypeStatement {
            kind,
            level,
            type_key: None,
            date: None,
        },
        name: Some(name.to_owned()),
    };
    vitni_app::create_place(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("create place")
}

fn places(filters: Vec<ListFilter>) -> ListQuery {
    ListQuery {
        filters,
        ..ListQuery::new(ListedKind::Place)
    }
}

async fn listed_places(ws: &Workspace, query: &ListQuery) -> Result<Vec<String>, AppError> {
    let page = list_places_page(ws, query, ListPage::ALL).await?;
    Ok(page.items.into_iter().map(|summary| summary.human_id).collect())
}

#[tokio::test]
async fn place_kind_and_level_filters_take_their_variants_and_keep_unstated_fields_per_none_of() {
    let (ws, _dir) = workspace().await;
    let haugen = classified_place(&ws, "Haugen", Some(PlaceKind::Settlement), Some(PlaceLevel::Settlement)).await;
    let norge = classified_place(&ws, "Norge", Some(PlaceKind::Administrative), Some(PlaceLevel::Country)).await;
    let hamar = classified_place(&ws, "Hamar", Some(PlaceKind::Ecclesiastical), Some(PlaceLevel::Adm1)).await;
    let bare = classified_place(&ws, "Ringsaker", None, None).await;
    let values = |items: &[&str]| FilterValue::Values(items.iter().map(|item| (*item).to_owned()).collect());

    let settlements = places(vec![filter("kind", FilterOp::IsAnyOf, values(&["settlement"]))]);
    assert_eq!(
        listed_places(&ws, &settlements).await.expect("list"),
        std::slice::from_ref(&haugen)
    );
    let not_country = places(vec![filter("level", FilterOp::IsNoneOf, values(&["country"]))]);
    let mut expected = vec![haugen, hamar.clone(), bare];
    expected.sort();
    let mut found = listed_places(&ws, &not_country).await.expect("list");
    found.sort();
    assert_eq!(
        found, expected,
        "none of keeps the place with no level, and drops {norge}"
    );
    let adm1 = places(vec![filter("level", FilterOp::IsAnyOf, values(&["Adm1"]))]);
    assert_eq!(listed_places(&ws, &adm1).await.expect("list"), [hamar]);

    let typo = places(vec![filter("level", FilterOp::IsAnyOf, values(&["adm9"]))]);
    let refused = listed_places(&ws, &typo).await;
    assert!(
        matches!(
            refused,
            Err(AppError::ListQuery(ListQueryError::UnknownChoice { ref column, ref value }))
                if column == "level" && value == "adm9"
        ),
        "{refused:?}"
    );
    let crossed = places(vec![filter("kind", FilterOp::IsAnyOf, values(&["adm1"]))]);
    assert!(
        matches!(
            listed_places(&ws, &crossed).await,
            Err(AppError::ListQuery(ListQueryError::UnknownChoice { .. }))
        ),
        "a level is not a kind"
    );
}
