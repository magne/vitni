//! Integration tests for the list tables and the list query (ADR 0052 §4–5), through the public `Store`
//! API, on SQLite and (under `--features postgres`, with a Docker daemon) Postgres: every test body is
//! one `async fn` over a `&Store`, run once per engine.

#![cfg(any(feature = "sqlite", feature = "postgres"))]
#![expect(clippy::unwrap_used, reason = "tests abort on setup/assertion failure")]

use time::macros::datetime;
use uuid::Uuid;
use vitni_core::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use vitni_core::enums::{EventType, EvidenceLevel, ParticipantRole, PlaceType, Sex};
use vitni_core::event::command::{EventCommand, EventCommandEnvelope};
use vitni_core::family::command::{FamilyCommand, FamilyCommandEnvelope};
use vitni_core::ids::{AgentId, AssertionId, EventId, FamilyId, HumanId, PersonId, PlaceId, TagId};
use vitni_core::name::{NameType, PersonName, Surname};
use vitni_core::person::command::{PersonCommand, PersonCommandEnvelope};
use vitni_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
use vitni_core::place_name::PlaceName;
use vitni_core::provenance::{Agent, AgentKind, AssertionMeta, EventContext, Timestamp};
use vitni_core::tag::command::{TagCommand, TagCommandEnvelope};
use vitni_db::{
    ListCell, ListPredicate, ListResult, ListSort, ListSpec, ListTest, ListWindow, ListedKind, SortDirection, Store,
};

fn meta(assertion: u128) -> AssertionMeta {
    AssertionMeta {
        assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
        context: EventContext {
            operator: Agent {
                kind: AgentKind::Human,
                id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                display: None,
            },
            occurred_at: Timestamp::new(datetime!(2026-10-18 12:00:00 UTC)),
            rationale: None,
            confidence: None,
            citations: Vec::new(),
            evidence_analysis: None,
            origin: None,
        },
    }
}

fn id(n: u128) -> Uuid {
    Uuid::from_u128(n)
}

fn year(value: i32) -> GenealogicalDate {
    GenealogicalDate {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(value),
            month: None,
            day: None,
        })),
        time: None,
        new_year_begins: None,
        sort_value: i64::from(value) * 10_000,
        original_text: None,
    }
}

fn text_date(text: &str) -> GenealogicalDate {
    GenealogicalDate {
        modifier: GenealogicalDateBody::TextOnly { text: text.to_owned() },
        sort_value: 0,
        ..year(0)
    }
}

async fn person(store: &Store, n: u128, assertion: u128, command: PersonCommand) {
    let envelope = PersonCommandEnvelope {
        meta: meta(assertion),
        command,
    };
    store.execute_person(&id(n).to_string(), envelope).await.unwrap();
}

fn person_id(n: u128) -> PersonId {
    PersonId::from_uuid(id(n))
}

/// Creates person `n` as `I<n>` named `given surname`.
async fn create_person(store: &Store, n: u128, given: &str, surname: &str) {
    let create = PersonCommand::CreatePerson {
        person_id: person_id(n),
        human_id: HumanId::new(format!("I{n:04}")),
        evidence_level: EvidenceLevel::Conclusion,
        external_ids: Vec::new(),
    };
    person(store, n, n * 100, create).await;
    let name = PersonName {
        name_type: NameType::BirthName,
        given: Some(given.to_owned()),
        surnames: vec![Surname {
            prefix: None,
            surname: surname.to_owned(),
            primary: true,
            connector: None,
        }],
        suffix: None,
        title: None,
        nickname: None,
        call_name: None,
        date: None,
        language: None,
        transliterations: Vec::new(),
    };
    let assert = PersonCommand::AssertName {
        person_id: person_id(n),
        name,
    };
    person(store, n, n * 100 + 1, assert).await;
}

async fn set_sex(store: &Store, n: u128, sex: Sex) {
    let command = PersonCommand::AssertSex {
        person_id: person_id(n),
        sex,
    };
    person(store, n, n * 100 + 2, command).await;
}

async fn event(store: &Store, n: u128, assertion: u128, command: EventCommand) {
    let envelope = EventCommandEnvelope {
        meta: meta(assertion),
        command,
    };
    store.execute_event(&id(n).to_string(), envelope).await.unwrap();
}

/// Creates event `n` of `event_type` on `date`, with person `primary` its primary participant.
async fn vital(store: &Store, n: u128, event_type: EventType, date: GenealogicalDate, primary: u128) {
    let create = EventCommand::CreateEvent {
        event_id: EventId::from_uuid(id(n)),
        human_id: HumanId::new(format!("E{n:04}")),
        event_type,
    };
    event(store, n, n * 100, create).await;
    let date = EventCommand::AssertDate {
        event_id: EventId::from_uuid(id(n)),
        date,
    };
    event(store, n, n * 100 + 1, date).await;
    let take_part = PersonCommand::AssertParticipation {
        person_id: person_id(primary),
        event_id: EventId::from_uuid(id(n)),
        role: ParticipantRole::Primary,
        age: None,
        attributes: Vec::new(),
        notes: Vec::new(),
    };
    person(store, primary, n * 100 + 2, take_part).await;
}

async fn create_place(store: &Store, n: u128, name: &str) {
    let create = PlaceCommand::CreatePlace {
        place_id: PlaceId::from_uuid(id(n)),
        human_id: HumanId::new(format!("P{n:04}")),
        place_type: PlaceType::Parish,
    };
    place(store, n, n * 100, create).await;
    name_place(store, n, n * 100 + 1, name).await;
}

async fn name_place(store: &Store, n: u128, assertion: u128, name: &str) {
    let command = PlaceCommand::AssertName {
        place_id: PlaceId::from_uuid(id(n)),
        name: PlaceName {
            text: name.to_owned(),
            language: None,
            date: None,
        },
    };
    place(store, n, assertion, command).await;
}

async fn place(store: &Store, n: u128, assertion: u128, command: PlaceCommand) {
    let envelope = PlaceCommandEnvelope {
        meta: meta(assertion),
        command,
    };
    store.execute_place(&id(n).to_string(), envelope).await.unwrap();
}

async fn create_tag(store: &Store, n: u128) {
    let envelope = TagCommandEnvelope {
        meta: meta(n * 100),
        command: TagCommand::CreateTag {
            tag_id: TagId::from_uuid(id(n)),
            name: format!("tag {n}"),
        },
    };
    store.execute_tag(&id(n).to_string(), envelope).await.unwrap();
}

async fn tag_person(store: &Store, n: u128, tag: u128) {
    let command = PersonCommand::Tag {
        person_id: person_id(n),
        tag_id: TagId::from_uuid(id(tag)),
    };
    person(store, n, n * 100 + tag, command).await;
}

fn spec(kind: ListedKind) -> ListSpec {
    ListSpec {
        kind,
        search: None,
        filters: Vec::new(),
        sort: None,
        columns: Vec::new(),
        window: ListWindow::Offset { offset: 0, limit: None },
    }
}

fn filtered(kind: ListedKind, column: &str, test: ListTest) -> ListSpec {
    ListSpec {
        filters: vec![ListPredicate {
            column: column.to_owned(),
            test,
        }],
        ..spec(kind)
    }
}

async fn run(store: &Store, spec: &ListSpec) -> ListResult {
    store.list_query(spec).await.unwrap()
}

/// The `human_id`s a query returns, in its order.
async fn human_ids(store: &Store, spec: &ListSpec) -> Vec<String> {
    run(store, spec)
        .await
        .rows
        .into_iter()
        .map(|row| row.human_id.unwrap_or_default())
        .collect()
}

/// Five people: Ola (1790–1850, male), Kari (1810, female), Per (no dates), Åse Ødegård (born in a
/// text-only date), Nils (1790, male).
async fn five_people(store: &Store) {
    create_person(store, 1, "Ola", "Olsen").await;
    create_person(store, 2, "Kari", "Olsen").await;
    create_person(store, 3, "Per", "Hansen").await;
    create_person(store, 4, "Åse", "Ødegård").await;
    create_person(store, 5, "Nils", "Berg").await;
    set_sex(store, 1, Sex::Male).await;
    set_sex(store, 2, Sex::Female).await;
    set_sex(store, 5, Sex::Male).await;
    vital(store, 11, EventType::Birth, year(1790), 1).await;
    vital(store, 12, EventType::Death, year(1850), 1).await;
    vital(store, 21, EventType::Birth, year(1810), 2).await;
    vital(store, 41, EventType::Birth, text_date("the winter the river froze"), 4).await;
    vital(store, 51, EventType::Birth, year(1790), 5).await;
}

async fn a_query_pages_the_listed_records_with_their_total(store: &Store) {
    five_people(store).await;
    let merge = PersonCommand::MergePersons {
        surviving: person_id(1),
        merged: person_id(3),
        assessment: None,
    };
    person(store, 1, 199, merge).await;

    let all = run(store, &spec(ListedKind::Person)).await;
    assert_eq!(all.total, Some(4), "a merged record is not counted");
    let window = ListSpec {
        window: ListWindow::Offset {
            offset: 1,
            limit: Some(2),
        },
        ..spec(ListedKind::Person)
    };
    let page = run(store, &window).await;
    assert_eq!(page.total, Some(4), "the total counts every match, not the page");
    assert_eq!(
        page.rows
            .iter()
            .map(|row| row.human_id.clone().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["I0002", "I0004"],
        "human_id order, the merged I0003 skipped"
    );
    let past = ListSpec {
        window: ListWindow::Offset {
            offset: 9,
            limit: Some(2),
        },
        ..spec(ListedKind::Person)
    };
    let past = run(store, &past).await;
    assert!(past.rows.is_empty(), "no rows past the end");
    assert_eq!(past.total, Some(4), "a window past the end still counts the matches");
}

async fn a_row_carries_typed_cells_for_the_requested_columns(store: &Store) {
    five_people(store).await;
    create_tag(store, 70).await;
    tag_person(store, 1, 70).await;
    let columns = ListSpec {
        columns: ["birth", "name", "sex", "death", "tags"].map(str::to_owned).to_vec(),
        ..filtered(ListedKind::Person, "given", ListTest::Is("ola".to_owned()))
    };
    let result = run(store, &columns).await;
    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].cells,
        vec![
            Some(ListCell::Date(year(1790))),
            Some(ListCell::Text("Ola Olsen".to_owned())),
            Some(ListCell::Choice("male".to_owned())),
            Some(ListCell::Date(year(1850))),
            Some(ListCell::Tags(vec![TagId::from_uuid(id(70))])),
        ]
    );
    let per = ListSpec {
        columns: ["birth", "sex", "tags"].map(str::to_owned).to_vec(),
        ..filtered(ListedKind::Person, "given", ListTest::Is("Per".to_owned()))
    };
    assert_eq!(
        run(store, &per).await.rows[0].cells,
        vec![None, None, Some(ListCell::Tags(Vec::new()))],
        "an absent value is an empty cell; no tags is an empty set"
    );
}

async fn text_filters_fold_case_and_search_spans_the_text_columns(store: &Store) {
    five_people(store).await;
    let contains = filtered(ListedKind::Person, "surname", ListTest::Contains("ØDEG".to_owned()));
    assert_eq!(human_ids(store, &contains).await, ["I0004"], "folded beyond ASCII");
    let is = filtered(ListedKind::Person, "surname", ListTest::Is("olsen".to_owned()));
    assert_eq!(human_ids(store, &is).await, ["I0001", "I0002"]);
    let partial = filtered(ListedKind::Person, "surname", ListTest::Is("olse".to_owned()));
    assert!(
        human_ids(store, &partial).await.is_empty(),
        "is matches the whole value"
    );

    let search = ListSpec {
        search: Some("BERG".to_owned()),
        ..spec(ListedKind::Person)
    };
    assert_eq!(human_ids(store, &search).await, ["I0005"]);
    let by_id = ListSpec {
        search: Some("i0002".to_owned()),
        ..spec(ListedKind::Person)
    };
    assert_eq!(human_ids(store, &by_id).await, ["I0002"], "search matches the human_id");
    let label = ListSpec {
        search: Some("female".to_owned()),
        ..spec(ListedKind::Person)
    };
    assert!(
        human_ids(store, &label).await.is_empty(),
        "search does not match a choice value"
    );
}

async fn date_filters_compare_the_sort_key_and_skip_text_only_dates(store: &Store) {
    five_people(store).await;
    let before = filtered(ListedKind::Person, "birth", ListTest::Before(18_000_000));
    assert_eq!(human_ids(store, &before).await, ["I0001", "I0005"]);
    let after = filtered(ListedKind::Person, "birth", ListTest::After(17_901_231));
    assert_eq!(human_ids(store, &after).await, ["I0002"]);
    let between = filtered(
        ListedKind::Person,
        "birth",
        ListTest::DatesBetween(17_900_000, 17_901_231),
    );
    assert_eq!(human_ids(store, &between).await, ["I0001", "I0005"]);
    let empty = filtered(ListedKind::Person, "birth", ListTest::IsEmpty);
    assert_eq!(
        human_ids(store, &empty).await,
        ["I0003", "I0004"],
        "a text-only date has no sort key"
    );
}

async fn choice_and_tag_filters_match_sets(store: &Store) {
    five_people(store).await;
    create_tag(store, 70).await;
    create_tag(store, 71).await;
    tag_person(store, 1, 70).await;
    tag_person(store, 1, 71).await;
    tag_person(store, 2, 70).await;

    let any = filtered(
        ListedKind::Person,
        "sex",
        ListTest::AnyOf(vec!["female".to_owned(), "male".to_owned()]),
    );
    assert_eq!(human_ids(store, &any).await, ["I0001", "I0002", "I0005"]);
    let none = filtered(ListedKind::Person, "sex", ListTest::NoneOf(vec!["male".to_owned()]));
    assert_eq!(
        human_ids(store, &none).await,
        ["I0002", "I0003", "I0004"],
        "none of keeps the records with no value"
    );

    let tag = |n: u128| id(n).to_string();
    let any = filtered(ListedKind::Person, "tags", ListTest::AnyOf(vec![tag(70)]));
    assert_eq!(human_ids(store, &any).await, ["I0001", "I0002"]);
    let all = filtered(ListedKind::Person, "tags", ListTest::AllOf(vec![tag(70), tag(71)]));
    assert_eq!(human_ids(store, &all).await, ["I0001"]);
    let none = filtered(ListedKind::Person, "tags", ListTest::NoneOf(vec![tag(71)]));
    assert_eq!(human_ids(store, &none).await, ["I0002", "I0003", "I0004", "I0005"]);
    let nothing = filtered(ListedKind::Person, "tags", ListTest::AnyOf(Vec::new()));
    assert!(
        human_ids(store, &nothing).await.is_empty(),
        "any of no tag matches nothing"
    );
    let everyone = filtered(ListedKind::Person, "tags", ListTest::NoneOf(Vec::new()));
    assert_eq!(
        human_ids(store, &everyone).await.len(),
        5,
        "none of no tag matches everyone"
    );
}

async fn a_sort_orders_by_the_column_then_human_id_with_empty_values_last(store: &Store) {
    five_people(store).await;
    let sorted = |direction| ListSpec {
        sort: Some(ListSort {
            column: "birth".to_owned(),
            direction,
        }),
        ..spec(ListedKind::Person)
    };
    assert_eq!(
        human_ids(store, &sorted(SortDirection::Ascending)).await,
        ["I0001", "I0005", "I0002", "I0003", "I0004"]
    );
    assert_eq!(
        human_ids(store, &sorted(SortDirection::Descending)).await,
        ["I0002", "I0001", "I0005", "I0003", "I0004"],
        "ties stay in human_id order, empty values stay last"
    );
}

async fn a_keyset_walk_reads_the_same_rows_as_offset_pages(store: &Store) {
    five_people(store).await;
    for direction in [SortDirection::Ascending, SortDirection::Descending] {
        for column in ["birth", "surname", "sex"] {
            let sorted = ListSpec {
                sort: Some(ListSort {
                    column: column.to_owned(),
                    direction,
                }),
                ..spec(ListedKind::Person)
            };
            let whole = human_ids(store, &sorted).await;
            let mut walked = Vec::new();
            let mut window = ListWindow::Offset {
                offset: 0,
                limit: Some(2),
            };
            loop {
                let page = run(
                    store,
                    &ListSpec {
                        window: window.clone(),
                        ..sorted.clone()
                    },
                )
                .await;
                let Some(last) = page.rows.last() else { break };
                assert_eq!(
                    page.total.is_some(),
                    matches!(window, ListWindow::Offset { .. }),
                    "only an offset window counts"
                );
                window = ListWindow::After {
                    cursor: last.cursor.clone(),
                    limit: Some(2),
                };
                walked.extend(page.rows.iter().map(|row| row.human_id.clone().unwrap_or_default()));
            }
            assert_eq!(walked, whole, "{column} {direction:?}");
        }
    }
}

async fn a_referenced_records_change_reaches_its_dependents_rows(store: &Store) {
    five_people(store).await;
    let date = EventCommand::AssertDate {
        event_id: EventId::from_uuid(id(21)),
        date: year(1700),
    };
    event(store, 21, 2_199, date).await;
    let before = filtered(ListedKind::Person, "birth", ListTest::Before(17_500_000));
    assert_eq!(
        human_ids(store, &before).await,
        ["I0002"],
        "an event's new date reaches its person"
    );

    create_place(store, 30, "Fana").await;
    let link = EventCommand::LinkPlace {
        event_id: EventId::from_uuid(id(11)),
        place_id: PlaceId::from_uuid(id(30)),
    };
    event(store, 11, 1_199, link).await;
    let retract = PlaceCommand::RetractAssertion {
        place_id: PlaceId::from_uuid(id(30)),
        target: AssertionId::from_uuid(id(3_001)),
    };
    place(store, 30, 3_002, retract).await;
    name_place(store, 30, 3_003, "Fana prestegjeld").await;
    let events = ListSpec {
        columns: vec!["place".to_owned()],
        ..filtered(ListedKind::Event, "place", ListTest::Is(id(30).to_string()))
    };
    let result = run(store, &events).await;
    assert_eq!(result.rows.len(), 1);
    assert_eq!(
        result.rows[0].cells,
        vec![Some(ListCell::Record {
            id: id(30).to_string(),
            title: Some("Fana prestegjeld".to_owned()),
        })],
        "a renamed place reaches its event's row"
    );
}

async fn a_family_row_reads_its_partners_marriage_and_children(store: &Store) {
    five_people(store).await;
    let create = FamilyCommand::CreateFamily {
        family_id: FamilyId::from_uuid(id(60)),
        human_id: HumanId::new("F0060"),
        external_ids: Vec::new(),
    };
    family(store, 6_000, create).await;
    for (assertion, command) in [
        (
            6_001,
            FamilyCommand::AddPartner {
                family_id: FamilyId::from_uuid(id(60)),
                person_id: person_id(1),
            },
        ),
        (
            6_002,
            FamilyCommand::AddPartner {
                family_id: FamilyId::from_uuid(id(60)),
                person_id: person_id(2),
            },
        ),
        (
            6_003,
            FamilyCommand::AddChild {
                family_id: FamilyId::from_uuid(id(60)),
                child_id: person_id(3),
            },
        ),
    ] {
        family(store, assertion, command).await;
    }
    let create = EventCommand::CreateEvent {
        event_id: EventId::from_uuid(id(61)),
        human_id: HumanId::new("E0061"),
        event_type: EventType::Marriage,
    };
    event(store, 61, 6_100, create).await;
    let date = EventCommand::AssertDate {
        event_id: EventId::from_uuid(id(61)),
        date: year(1830),
    };
    event(store, 61, 6_101, date).await;
    let link = FamilyCommand::LinkFamilyEvent {
        family_id: FamilyId::from_uuid(id(60)),
        event_id: EventId::from_uuid(id(61)),
    };
    family(store, 6_004, link).await;

    let families = ListSpec {
        columns: ["partners", "marriage", "children"].map(str::to_owned).to_vec(),
        ..spec(ListedKind::Family)
    };
    assert_eq!(
        run(store, &families).await.rows[0].cells,
        vec![
            Some(ListCell::Text("Ola Olsen & Kari Olsen".to_owned())),
            Some(ListCell::Date(year(1830))),
            Some(ListCell::Number(1.0)),
        ]
    );
    let more = filtered(ListedKind::Family, "children", ListTest::GreaterThan(0.5));
    assert_eq!(human_ids(store, &more).await, ["F0060"]);
    let fewer = filtered(ListedKind::Family, "children", ListTest::LessThan(1.0));
    assert!(
        human_ids(store, &fewer).await.is_empty(),
        "no family has fewer than one child"
    );
}

async fn family(store: &Store, assertion: u128, command: FamilyCommand) {
    let envelope = FamilyCommandEnvelope {
        meta: meta(assertion),
        command,
    };
    store.execute_family(&id(60).to_string(), envelope).await.unwrap();
}

async fn a_filter_that_does_not_fit_its_column_is_refused(store: &Store) {
    five_people(store).await;
    for spec in [
        filtered(ListedKind::Person, "nickname", ListTest::IsEmpty),
        filtered(ListedKind::Person, "birth", ListTest::Contains("17".to_owned())),
        ListSpec {
            sort: Some(ListSort {
                column: "nickname".to_owned(),
                direction: SortDirection::Ascending,
            }),
            ..spec(ListedKind::Person)
        },
        ListSpec {
            columns: vec!["nickname".to_owned()],
            ..spec(ListedKind::Person)
        },
    ] {
        assert!(store.list_query(&spec).await.is_err(), "{spec:?}");
    }
}

async fn a_rebuild_reproduces_the_rows(store: &Store) {
    five_people(store).await;
    let everything = ListSpec {
        columns: ["name", "given", "surname", "sex", "birth", "death"]
            .map(str::to_owned)
            .to_vec(),
        ..spec(ListedKind::Person)
    };
    let live = run(store, &everything).await;
    store.rebuild_projections().await.unwrap();
    assert_eq!(run(store, &everything).await, live);
}

async fn every_kind_lists_with_every_column(store: &Store) {
    five_people(store).await;
    create_tag(store, 70).await;
    create_place(store, 30, "Fana").await;
    for kind in ListedKind::ALL {
        let all = ListSpec {
            columns: kind.columns().iter().map(|column| column.id.to_owned()).collect(),
            ..spec(*kind)
        };
        let result = run(store, &all).await;
        for row in &result.rows {
            assert_eq!(row.cells.len(), kind.columns().len(), "{kind:?}");
        }
    }
    assert_eq!(run(store, &spec(ListedKind::Tag)).await.total, Some(1));
    assert_eq!(run(store, &spec(ListedKind::Place)).await.total, Some(1));
    assert_eq!(run(store, &spec(ListedKind::Event)).await.total, Some(5));
}

async fn a_tags_uses_are_read_through_its_links_a_cluster_once(store: &Store) {
    five_people(store).await;
    create_tag(store, 70).await;
    create_place(store, 30, "Fana").await;
    for n in [1, 3, 5] {
        tag_person(store, n, 70).await;
    }
    let tag_place = PlaceCommand::Tag {
        place_id: PlaceId::from_uuid(id(30)),
        tag_id: TagId::from_uuid(id(70)),
    };
    place(store, 30, 3_070, tag_place).await;
    let merge = PersonCommand::MergePersons {
        surviving: person_id(1),
        merged: person_id(3),
        assessment: None,
    };
    person(store, 1, 199, merge).await;

    let tag = id(70).to_string();
    assert_eq!(
        store.tag_uses(&tag).await.unwrap(),
        [
            (ListedKind::Person, id(1).to_string()),
            (ListedKind::Person, id(5).to_string()),
            (ListedKind::Place, id(30).to_string()),
        ],
        "by kind, then human_id; the merged I0003 read as its root I0001, once"
    );
    assert_eq!(store.tag_use_counts().await.unwrap(), [(tag, 3)]);
    assert!(
        store.tag_uses(&id(71).to_string()).await.unwrap().is_empty(),
        "an unused tag"
    );
}

async fn a_cluster_root_carries_its_members_tags_and_references(store: &Store) {
    five_people(store).await;
    create_tag(store, 70).await;
    tag_person(store, 3, 70).await;
    let merge = PersonCommand::MergePersons {
        surviving: person_id(1),
        merged: person_id(3),
        assessment: None,
    };
    person(store, 1, 199, merge).await;
    let tagged = ListSpec {
        columns: vec!["tags".to_owned()],
        ..filtered(ListedKind::Person, "tags", ListTest::AnyOf(vec![id(70).to_string()]))
    };
    let result = run(store, &tagged).await;
    assert_eq!(
        result
            .rows
            .iter()
            .map(|row| row.human_id.clone().unwrap_or_default())
            .collect::<Vec<_>>(),
        ["I0001"],
        "the root carries its member's tag, as the tag's uses count it"
    );
    assert_eq!(
        result.rows[0].cells,
        vec![Some(ListCell::Tags(vec![TagId::from_uuid(id(70))]))]
    );
    let none = filtered(ListedKind::Person, "tags", ListTest::NoneOf(vec![id(70).to_string()]));
    assert!(!human_ids(store, &none).await.contains(&"I0001".to_owned()));

    create_place(store, 30, "Fana").await;
    create_place(store, 31, "Fana sokn").await;
    let link = EventCommand::LinkPlace {
        event_id: EventId::from_uuid(id(11)),
        place_id: PlaceId::from_uuid(id(31)),
    };
    event(store, 11, 1_199, link).await;
    let merge = PlaceCommand::MergePlaces {
        surviving: PlaceId::from_uuid(id(30)),
        merged: PlaceId::from_uuid(id(31)),
        assessment: None,
    };
    place(store, 30, 3_099, merge).await;
    let at_root = filtered(ListedKind::Event, "place", ListTest::Is(id(30).to_string()));
    assert_eq!(
        human_ids(store, &at_root).await,
        ["E0011"],
        "an event at a merged-in place is at its root"
    );
}

#[cfg(feature = "sqlite")]
mod sqlite {
    use vitni_db::{ListTest, ListedKind, RecordLink, Store};

    async fn store() -> (Store, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("ws.sqlite3").display());
        (Store::open(&url).await.unwrap(), dir)
    }

    macro_rules! sqlite_tests {
        ($($name:ident),+ $(,)?) => {
            $(
                #[tokio::test]
                async fn $name() {
                    let (store, _dir) = store().await;
                    super::$name(&store).await;
                }
            )+
        };
    }

    sqlite_tests!(
        a_cluster_root_carries_its_members_tags_and_references,
        a_tags_uses_are_read_through_its_links_a_cluster_once,
        a_query_pages_the_listed_records_with_their_total,
        a_row_carries_typed_cells_for_the_requested_columns,
        text_filters_fold_case_and_search_spans_the_text_columns,
        date_filters_compare_the_sort_key_and_skip_text_only_dates,
        choice_and_tag_filters_match_sets,
        a_sort_orders_by_the_column_then_human_id_with_empty_values_last,
        a_keyset_walk_reads_the_same_rows_as_offset_pages,
        a_referenced_records_change_reaches_its_dependents_rows,
        a_family_row_reads_its_partners_marriage_and_children,
        a_filter_that_does_not_fit_its_column_is_refused,
        a_rebuild_reproduces_the_rows,
        every_kind_lists_with_every_column,
    );

    /// A workspace from before the list tables: none of them, and no `tagged` links.
    #[tokio::test]
    async fn a_workspace_opened_without_the_list_tables_fills_them_and_its_tag_links() {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("ws.sqlite3").display());
        let store = Store::open(&url).await.unwrap();
        super::five_people(&store).await;
        super::create_tag(&store, 70).await;
        super::tag_person(&store, 2, 70).await;
        drop(store);

        let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
        for kind in ListedKind::ALL {
            sqlx::query(&format!("DROP TABLE {}", kind.table()))
                .execute(&pool)
                .await
                .unwrap();
        }
        sqlx::query("DELETE FROM record_links WHERE relation = 'tagged'")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;

        let store = Store::open(&url).await.unwrap();
        let born = super::filtered(ListedKind::Person, "birth", ListTest::Before(18_000_000));
        assert_eq!(super::human_ids(&store, &born).await, ["I0001", "I0005"]);
        let tagged = store
            .linking(RecordLink::Tagged, &[super::id(70).to_string()])
            .await
            .unwrap();
        assert_eq!(tagged.len(), 1, "the tag links are filled with the list tables");
    }
}

#[cfg(feature = "postgres")]
mod postgres {
    use vitni_db::Store;
    use vitni_test_postgres::PostgresTestDb;

    async fn store() -> (Store, PostgresTestDb) {
        let db = PostgresTestDb::create("vitni-pg").await.unwrap();
        let store = Store::open(db.dsn()).await.unwrap();
        (store, db)
    }

    macro_rules! postgres_tests {
        ($($name:ident),+ $(,)?) => {
            $(
                #[tokio::test]
                async fn $name() {
                    let (store, _db) = store().await;
                    super::$name(&store).await;
                }
            )+
        };
    }

    postgres_tests!(
        a_cluster_root_carries_its_members_tags_and_references,
        a_tags_uses_are_read_through_its_links_a_cluster_once,
        a_query_pages_the_listed_records_with_their_total,
        a_row_carries_typed_cells_for_the_requested_columns,
        text_filters_fold_case_and_search_spans_the_text_columns,
        date_filters_compare_the_sort_key_and_skip_text_only_dates,
        choice_and_tag_filters_match_sets,
        a_sort_orders_by_the_column_then_human_id_with_empty_values_last,
        a_keyset_walk_reads_the_same_rows_as_offset_pages,
        a_referenced_records_change_reaches_its_dependents_rows,
        a_family_row_reads_its_partners_marriage_and_children,
        a_filter_that_does_not_fit_its_column_is_refused,
        a_rebuild_reproduces_the_rows,
        every_kind_lists_with_every_column,
    );
}
