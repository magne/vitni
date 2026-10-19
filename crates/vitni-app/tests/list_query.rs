//! List queries (ADR 0052 §1–4, §10): a typed, serde `ListQuery` validated against the kind's column
//! catalogue and evaluated by the store, and the paged list use-cases that take one.

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    AppDefaults, AppError, DateParts, FilterOp, FilterValue, ListCell, ListFilter, ListPage, ListQuery, ListQueryError,
    ListSort, ListWindow, ListedKind, MutationMeta, NewPerson, OperatorConfig, PersonNameParts, Provenance, Session,
    SortDirection, Workspace, WorkspaceDefaults, assert_sex, create_person, create_tag, gregorian_date,
    list_persons_page, list_places_page, query_list, record_birth, tag_person,
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
        columns: vec!["name".to_owned(), "birth".to_owned()],
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

#[test]
fn a_query_serializes_by_column_and_operator_name() {
    let query = ListQuery {
        search: "ola".to_owned(),
        sort: Some(ListSort {
            column: "birth".to_owned(),
            direction: SortDirection::Descending,
        }),
        columns: vec!["birth".to_owned()],
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
