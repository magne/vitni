//! The Explorer's paged read (ADR 0052 §4): a page of the rows a store-evaluated `ListQuery` lists, at
//! its offset, under the total it matched — and the tag list read whole.
//!
//! Runs against a real on-disk workspace over `vitni-app`'s public surface only (ADR 0008).

#![expect(clippy::expect_used, reason = "tests abort on setup failure")]

use uuid::Uuid;
use vitni_app::{
    Agent, AgentId, AgentKind, AppDefaults, AppError, ColumnType, EventType, EvidenceLevel, FilterOp, FilterValue,
    ListFilter, ListQuery, ListQueryError, ListSort, ListWindow, ListedKind, MutationMeta, NewEvent, NewPerson,
    NewPlace, OperatorConfig, PersonNameParts, PlaceType, Provenance, Session, SortDirection, Workspace,
    WorkspaceDefaults, create_event, create_person, create_place, create_tag, link_place, query_list, tag_person,
};
use vitni_ui::{FilterDraft, Localizer, PAGE_ROWS, choice_options, list_page, resolve_filter};

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

async fn setup() -> (Workspace, Localizer, tempfile::TempDir) {
    let dir = tempfile::tempdir().expect("tempdir");
    let ws = dir.path().join("ws");
    Workspace::init(&ws, &operator(), &AppDefaults::default(), None).expect("init");
    let workspace = Workspace::open(&ws, &operator(), &WorkspaceDefaults::default())
        .await
        .expect("open workspace");
    let loc = Localizer::for_workspace(&ws, None);
    (workspace, loc, dir)
}

async fn person(ws: &Workspace, given: &str, surname: &str) -> String {
    let new = NewPerson {
        human_id: None,
        name: Some(PersonNameParts::simple(
            Some(given.to_owned()),
            Some(surname.to_owned()),
        )),
        evidence_level: EvidenceLevel::Conclusion,
        external_ids: Vec::new(),
    };
    create_person(ws, &session(), new, Provenance::default(), &[])
        .await
        .expect("person")
}

fn titles(rows: &[vitni_ui::RowVm]) -> Vec<&str> {
    rows.iter().map(|row| row.title.as_str()).collect()
}

#[tokio::test]
async fn a_page_lists_the_query_as_rows_under_its_total() {
    let (ws, loc, _dir) = setup().await;
    person(&ws, "Berit", "Olsen").await;
    person(&ws, "Per", "Hansen").await;
    person(&ws, "Anne", "Olsen").await;
    let query = ListQuery {
        search: "olsen".to_owned(),
        filters: vec![ListFilter {
            column: "given".to_owned(),
            op: FilterOp::Contains,
            value: FilterValue::Text("anne".to_owned()),
        }],
        ..ListQuery::new(ListedKind::Person)
    };

    let page = list_page(&ws, &loc, &query, 0).await.expect("page");

    assert_eq!((page.offset, page.total), (0, 1));
    assert_eq!(titles(&page.rows), ["Anne Olsen"]);
}

#[tokio::test]
async fn a_later_page_starts_at_its_offset_in_the_sorted_result() {
    let (ws, loc, _dir) = setup().await;
    for index in 0..=PAGE_ROWS {
        person(&ws, &format!("Given{index:03}"), "Olsen").await;
    }
    let query = ListQuery {
        sort: Some(ListSort {
            column: "name".to_owned(),
            direction: SortDirection::Descending,
        }),
        ..ListQuery::new(ListedKind::Person)
    };

    let page = list_page(&ws, &loc, &query, 1).await.expect("page");

    assert_eq!((page.offset, page.total), (PAGE_ROWS, PAGE_ROWS + 1));
    assert_eq!(
        titles(&page.rows),
        ["Given000 Olsen"],
        "the last name in descending order"
    );
}

#[tokio::test]
async fn the_tag_list_is_read_whole_from_the_first_row() {
    let (ws, loc, _dir) = setup().await;
    for name in ["Verified", "Unsure", "To verify"] {
        create_tag(&ws, &session(), name.to_owned(), Provenance::default(), &[])
            .await
            .expect("tag");
    }
    let query = ListQuery {
        search: "verif".to_owned(),
        sort: Some(ListSort {
            column: "name".to_owned(),
            direction: SortDirection::Ascending,
        }),
        ..ListQuery::new(ListedKind::Tag)
    };

    let page = list_page(&ws, &loc, &query, 3).await.expect("tags");

    assert_eq!((page.offset, page.total), (0, 2));
    assert_eq!(titles(&page.rows), ["To verify", "Verified"]);
}

#[tokio::test]
async fn a_query_the_catalogue_cannot_evaluate_is_an_error() {
    let (ws, loc, _dir) = setup().await;
    let query = ListQuery {
        filters: vec![ListFilter {
            column: "shoe-size".to_owned(),
            op: FilterOp::Is,
            value: FilterValue::Text("42".to_owned()),
        }],
        ..ListQuery::new(ListedKind::Person)
    };

    let result = list_page(&ws, &loc, &query, 0).await;

    assert!(matches!(
        result,
        Err(AppError::ListQuery(ListQueryError::UnknownColumn { .. }))
    ));
}

#[tokio::test]
async fn the_store_takes_every_choice_the_filter_editor_offers() {
    let (ws, loc, _dir) = setup().await;
    for kind in ListedKind::ALL {
        for column in kind.columns().iter().filter(|column| column.ty == ColumnType::Choice) {
            let options = choice_options(&loc, *kind, column.id);
            assert!(!options.is_empty(), "{} {} offers no choice", kind.as_str(), column.id);
            let query = ListQuery {
                filters: vec![ListFilter {
                    column: column.id.to_owned(),
                    op: FilterOp::IsAnyOf,
                    value: FilterValue::Values(options.into_iter().map(|option| option.key).collect()),
                }],
                ..ListQuery::new(*kind)
            };
            let result = query_list(&ws, &query, ListWindow::Offset { offset: 0, limit: None }).await;
            assert!(result.is_ok(), "{} {}: {result:?}", kind.as_str(), column.id);
        }
    }
}

#[tokio::test]
async fn a_picked_record_is_resolved_to_the_id_the_query_holds() {
    let (ws, loc, _dir) = setup().await;
    let place = |name: &str| NewPlace {
        human_id: None,
        place_type: PlaceType::Parish,
        name: Some(name.to_owned()),
    };
    let fana = create_place(&ws, &session(), place("Fana"), Provenance::default(), &[])
        .await
        .expect("place");
    create_place(&ws, &session(), place("Os"), Provenance::default(), &[])
        .await
        .expect("place");
    let baptism = create_event(
        &ws,
        &session(),
        NewEvent {
            human_id: None,
            event_type: EventType::Baptism,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("event");
    create_event(
        &ws,
        &session(),
        NewEvent {
            human_id: None,
            event_type: EventType::Burial,
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("event");
    link_place(&ws, &session(), &baptism, &fana, MutationMeta::default())
        .await
        .expect("link");
    let mut draft = FilterDraft::new(ListedKind::Event).with_column(ListedKind::Event, "place");
    draft.values = vec![fana.clone()];
    let active = draft.apply(ListedKind::Event).expect("applies");

    let active = resolve_filter(&ws, ListedKind::Event, active).await.expect("resolves");
    let query = ListQuery {
        filters: vec![active.filter.clone()],
        ..ListQuery::new(ListedKind::Event)
    };
    let page = list_page(&ws, &loc, &query, 0).await.expect("page");

    assert_ne!(
        active.filter.value,
        FilterValue::Text(fana.clone()),
        "held by id, not human_id"
    );
    assert_eq!(active.draft.values, [fana], "the draft keeps what was picked");
    assert_eq!(
        page.rows.iter().map(|row| row.id.as_str()).collect::<Vec<_>>(),
        [baptism.as_str()]
    );
}

#[tokio::test]
async fn a_rows_second_line_is_its_chosen_columns_an_empty_cell_skipped() {
    let (ws, loc, _dir) = setup().await;
    let fana = create_place(
        &ws,
        &session(),
        NewPlace {
            human_id: None,
            place_type: PlaceType::Parish,
            name: Some("Fana".to_owned()),
        },
        Provenance::default(),
        &[],
    )
    .await
    .expect("place");
    let mut events = Vec::new();
    for event_type in [EventType::Baptism, EventType::Burial] {
        let new = NewEvent {
            human_id: None,
            event_type,
        };
        events.push(
            create_event(&ws, &session(), new, Provenance::default(), &[])
                .await
                .expect("event"),
        );
    }
    link_place(&ws, &session(), &events[0], &fana, MutationMeta::default())
        .await
        .expect("link");
    let query = ListQuery {
        columns: Some(vec!["description".to_owned(), "place".to_owned()]),
        ..ListQuery::new(ListedKind::Event)
    };

    let page = list_page(&ws, &loc, &query, 0).await.expect("page");

    let lines: Vec<Option<&str>> = page.rows.iter().map(|row| row.subtitle.as_deref()).collect();
    assert_eq!(lines, [Some("Fana"), None], "no description and no place leave no line");
    let bare = list_page(&ws, &loc, &ListQuery::new(ListedKind::Event), 0)
        .await
        .expect("bare page");
    assert!(
        bare.rows.iter().all(|row| row.subtitle.is_none()),
        "no columns, no line"
    );
}

#[tokio::test]
async fn a_tag_column_reads_the_tags_by_name() {
    let (ws, loc, _dir) = setup().await;
    let ola = person(&ws, "Ola", "Olsen").await;
    let tag = create_tag(&ws, &session(), "To verify".to_owned(), Provenance::default(), &[])
        .await
        .expect("tag");
    tag_person(&ws, &session(), &ola, &tag, false, MutationMeta::default())
        .await
        .expect("tag ola");
    let query = ListQuery {
        columns: Some(vec!["tags".to_owned()]),
        ..ListQuery::new(ListedKind::Person)
    };

    let page = list_page(&ws, &loc, &query, 0).await.expect("page");

    assert_eq!(page.rows[0].subtitle.as_deref(), Some("To verify"));
}
