//! Integration tests for the by-id view reads and the `record_links` index, through the public `Store`
//! API, on SQLite and (under `--features postgres`, with a Docker daemon) Postgres: every test body is
//! one `async fn` over a `&Store`, run once per engine.

#![cfg(any(feature = "sqlite", feature = "postgres"))]
#![expect(clippy::unwrap_used, reason = "tests abort on setup/assertion failure")]

use time::macros::datetime;
use uuid::Uuid;
use vitni_core::citation::command::{CitationCommand, CitationCommandEnvelope};
use vitni_core::enums::{EventType, EvidenceLevel, ParticipantRole, PlaceType, SourceMediaType};
use vitni_core::event::command::{EventCommand, EventCommandEnvelope};
use vitni_core::family::command::{FamilyCommand, FamilyCommandEnvelope};
use vitni_core::ids::{
    AgentId, AssertionId, CitationId, EventId, FamilyId, HumanId, PersonId, PlaceId, RepositoryId, SourceId, TagId,
};
use vitni_core::person::command::{PersonCommand, PersonCommandEnvelope};
use vitni_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
use vitni_core::place_ref::PlaceRef;
use vitni_core::provenance::{Agent, AgentKind, AssertionMeta, EventContext, Timestamp};
use vitni_core::repo_ref::RepoRef;
use vitni_core::repository::command::{RepositoryCommand, RepositoryCommandEnvelope};
use vitni_core::source::command::{SourceCommand, SourceCommandEnvelope};
use vitni_core::tag::command::{TagCommand, TagCommandEnvelope};
use vitni_db::{RecordLink, Store};

fn meta(assertion: u128) -> AssertionMeta {
    AssertionMeta {
        assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
        context: EventContext {
            operator: Agent {
                kind: AgentKind::Human,
                id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                display: None,
            },
            occurred_at: Timestamp::new(datetime!(2026-10-06 12:00:00 UTC)),
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

fn ids(of: &[u128]) -> Vec<String> {
    of.iter().map(|n| id(*n).to_string()).collect()
}

async fn person(store: &Store, n: u128, assertion: u128, command: PersonCommand) {
    let envelope = PersonCommandEnvelope {
        meta: meta(assertion),
        command,
    };
    store.execute_person(&id(n).to_string(), envelope).await.unwrap();
}

async fn create_person(store: &Store, n: u128) {
    let command = PersonCommand::CreatePerson {
        person_id: PersonId::from_uuid(id(n)),
        human_id: HumanId::new(format!("I{n:04}")),
        evidence_level: EvidenceLevel::Conclusion,
        external_ids: Vec::new(),
    };
    person(store, n, n * 100, command).await;
}

async fn take_part(store: &Store, n: u128, event: u128, assertion: u128) {
    let command = PersonCommand::AssertParticipation {
        person_id: PersonId::from_uuid(id(n)),
        event_id: EventId::from_uuid(id(event)),
        role: ParticipantRole::Primary,
        age: None,
        attributes: Vec::new(),
        notes: Vec::new(),
    };
    person(store, n, assertion, command).await;
}

async fn family(store: &Store, n: u128, assertion: u128, command: FamilyCommand) {
    let envelope = FamilyCommandEnvelope {
        meta: meta(assertion),
        command,
    };
    store.execute_family(&id(n).to_string(), envelope).await.unwrap();
}

async fn create_family(store: &Store, n: u128) {
    let command = FamilyCommand::CreateFamily {
        family_id: FamilyId::from_uuid(id(n)),
        human_id: HumanId::new(format!("F{n:04}")),
        external_ids: Vec::new(),
    };
    family(store, n, n * 100, command).await;
}

async fn event(store: &Store, n: u128, assertion: u128, command: EventCommand) {
    let envelope = EventCommandEnvelope {
        meta: meta(assertion),
        command,
    };
    store.execute_event(&id(n).to_string(), envelope).await.unwrap();
}

async fn create_event(store: &Store, n: u128) {
    let command = EventCommand::CreateEvent {
        event_id: EventId::from_uuid(id(n)),
        human_id: HumanId::new(format!("E{n:04}")),
        event_type: EventType::Birth,
    };
    event(store, n, n * 100, command).await;
}

async fn create_place(store: &Store, n: u128) {
    let envelope = PlaceCommandEnvelope {
        meta: meta(n * 100),
        command: PlaceCommand::CreatePlace {
            place_id: PlaceId::from_uuid(id(n)),
            human_id: HumanId::new(format!("P{n:04}")),
            place_type: PlaceType::Parish,
        },
    };
    store.execute_place(&id(n).to_string(), envelope).await.unwrap();
}

async fn enclose(store: &Store, n: u128, enclosing: u128, assertion: u128) {
    let envelope = PlaceCommandEnvelope {
        meta: meta(assertion),
        command: PlaceCommand::AssertEnclosedBy {
            place_id: PlaceId::from_uuid(id(n)),
            enclosed_by: PlaceRef {
                place_id: PlaceId::from_uuid(id(enclosing)),
                date: None,
            },
        },
    };
    store.execute_place(&id(n).to_string(), envelope).await.unwrap();
}

async fn create_repository(store: &Store, n: u128) {
    let envelope = RepositoryCommandEnvelope {
        meta: meta(n * 100),
        command: RepositoryCommand::CreateRepository {
            repository_id: RepositoryId::from_uuid(id(n)),
            human_id: HumanId::new(format!("R{n:04}")),
        },
    };
    store.execute_repository(&id(n).to_string(), envelope).await.unwrap();
}

async fn hold(store: &Store, source: u128, repository: u128, assertion: u128) {
    let envelope = SourceCommandEnvelope {
        meta: meta(assertion),
        command: SourceCommand::LinkRepository {
            source_id: SourceId::from_uuid(id(source)),
            repo_ref: RepoRef {
                repository_id: RepositoryId::from_uuid(id(repository)),
                call_number: None,
                media_type: SourceMediaType::Book,
            },
        },
    };
    store.execute_source(&id(source).to_string(), envelope).await.unwrap();
}

async fn create_source(store: &Store, n: u128) {
    let envelope = SourceCommandEnvelope {
        meta: meta(n * 100),
        command: SourceCommand::CreateSource {
            source_id: SourceId::from_uuid(id(n)),
            human_id: HumanId::new(format!("S{n:04}")),
        },
    };
    store.execute_source(&id(n).to_string(), envelope).await.unwrap();
}

async fn create_citation(store: &Store, n: u128, source: u128) {
    let envelope = CitationCommandEnvelope {
        meta: meta(n * 100),
        command: CitationCommand::CreateCitation {
            citation_id: CitationId::from_uuid(id(n)),
            human_id: HumanId::new(format!("C{n:04}")),
            source_id: SourceId::from_uuid(id(source)),
        },
    };
    store.execute_citation(&id(n).to_string(), envelope).await.unwrap();
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

/// The `(linking, linked)` pairs of `link` reaching any of `targets`, sorted.
async fn linking(store: &Store, link: RecordLink, targets: &[u128]) -> Vec<(String, String)> {
    let mut found = store.linking(link, &ids(targets)).await.unwrap();
    found.sort();
    found
}

fn pairs(of: &[(u128, u128)]) -> Vec<(String, String)> {
    let mut pairs: Vec<(String, String)> = of
        .iter()
        .map(|(source, target)| (id(*source).to_string(), id(*target).to_string()))
        .collect();
    pairs.sort();
    pairs
}

async fn views_are_read_by_id_skipping_unknown_ones(store: &Store) {
    for n in 1..=3 {
        create_person(store, n).await;
    }
    let mut read: Vec<String> = store
        .persons_by_ids(&ids(&[1, 3, 0x99]))
        .await
        .unwrap()
        .iter()
        .filter_map(|view| view.person_id().map(|id| id.to_string()))
        .collect();
    read.sort();
    assert_eq!(read, ids(&[1, 3]));
    assert_eq!(store.persons_by_ids(&[]).await.unwrap().len(), 0, "no ids, no views");
    assert_eq!(
        store.families_by_ids(&ids(&[1])).await.unwrap().len(),
        0,
        "another kind's table"
    );
}

async fn more_ids_than_one_query_binds_are_all_read(store: &Store) {
    let many: Vec<u128> = (1..=1_100).collect();
    for n in &many {
        create_tag(store, *n).await;
    }
    assert_eq!(store.tags_by_ids(&ids(&many)).await.unwrap().len(), many.len());
}

async fn a_familys_partners_and_children_link_to_it_while_they_are_members(store: &Store) {
    create_family(store, 10).await;
    for (assertion, command) in [
        (
            1001,
            FamilyCommand::AddPartner {
                family_id: FamilyId::from_uuid(id(10)),
                person_id: PersonId::from_uuid(id(1)),
            },
        ),
        (
            1002,
            FamilyCommand::AddPartner {
                family_id: FamilyId::from_uuid(id(10)),
                person_id: PersonId::from_uuid(id(2)),
            },
        ),
        (
            1003,
            FamilyCommand::AddChild {
                family_id: FamilyId::from_uuid(id(10)),
                child_id: PersonId::from_uuid(id(3)),
            },
        ),
    ] {
        family(store, 10, assertion, command).await;
    }
    assert_eq!(
        linking(store, RecordLink::FamilyPartner, &[1, 2, 3]).await,
        pairs(&[(10, 1), (10, 2)])
    );
    assert_eq!(
        linking(store, RecordLink::FamilyChild, &[1, 2, 3]).await,
        pairs(&[(10, 3)])
    );

    let remove = FamilyCommand::RemovePartner {
        family_id: FamilyId::from_uuid(id(10)),
        person_id: PersonId::from_uuid(id(2)),
    };
    family(store, 10, 1004, remove).await;
    assert_eq!(
        linking(store, RecordLink::FamilyPartner, &[1, 2, 3]).await,
        pairs(&[(10, 1)])
    );
    let retract = FamilyCommand::RetractAssertion {
        family_id: FamilyId::from_uuid(id(10)),
        target: AssertionId::from_uuid(id(1003)),
    };
    family(store, 10, 1005, retract).await;
    assert_eq!(linking(store, RecordLink::FamilyChild, &[3]).await, pairs(&[]));
}

async fn a_persons_participations_link_it_to_their_events_until_retracted(store: &Store) {
    create_event(store, 20).await;
    create_event(store, 21).await;
    create_person(store, 1).await;
    create_person(store, 2).await;
    take_part(store, 1, 20, 101).await;
    take_part(store, 1, 21, 102).await;
    take_part(store, 2, 20, 201).await;
    assert_eq!(
        linking(store, RecordLink::Participation, &[20]).await,
        pairs(&[(1, 20), (2, 20)])
    );

    let retract = PersonCommand::RetractAssertion {
        person_id: PersonId::from_uuid(id(1)),
        target: AssertionId::from_uuid(id(101)),
    };
    person(store, 1, 103, retract).await;
    assert_eq!(
        linking(store, RecordLink::Participation, &[20, 21]).await,
        pairs(&[(1, 21), (2, 20)])
    );
}

async fn an_event_links_to_its_place_and_a_citation_to_its_source(store: &Store) {
    create_place(store, 30).await;
    create_place(store, 31).await;
    create_event(store, 20).await;
    let link = |place: u128| EventCommand::LinkPlace {
        event_id: EventId::from_uuid(id(20)),
        place_id: PlaceId::from_uuid(id(place)),
    };
    event(store, 20, 2001, link(30)).await;
    assert_eq!(
        linking(store, RecordLink::EventPlace, &[30, 31]).await,
        pairs(&[(20, 30)])
    );
    event(store, 20, 2002, link(31)).await;
    assert_eq!(
        linking(store, RecordLink::EventPlace, &[30, 31]).await,
        pairs(&[(20, 31)]),
        "relinking moves the event"
    );

    create_source(store, 40).await;
    create_citation(store, 50, 40).await;
    assert_eq!(
        linking(store, RecordLink::CitationSource, &[40]).await,
        pairs(&[(50, 40)])
    );
    assert_eq!(linking(store, RecordLink::CitationSource, &[]).await, pairs(&[]));
}

async fn a_place_links_to_its_enclosing_places_until_retracted(store: &Store) {
    for n in [30, 31, 32] {
        create_place(store, n).await;
    }
    enclose(store, 30, 31, 3001).await;
    enclose(store, 30, 32, 3002).await;
    assert_eq!(
        linking(store, RecordLink::PlaceEnclosure, &[31, 32]).await,
        pairs(&[(30, 31), (30, 32)])
    );
    let envelope = PlaceCommandEnvelope {
        meta: meta(3003),
        command: PlaceCommand::RetractAssertion {
            place_id: PlaceId::from_uuid(id(30)),
            target: AssertionId::from_uuid(id(3001)),
        },
    };
    store.execute_place(&id(30).to_string(), envelope).await.unwrap();
    assert_eq!(
        linking(store, RecordLink::PlaceEnclosure, &[31, 32]).await,
        pairs(&[(30, 32)])
    );
}

async fn a_family_links_to_its_events_and_a_source_to_its_repositories(store: &Store) {
    create_event(store, 20).await;
    create_family(store, 10).await;
    let link = FamilyCommand::LinkFamilyEvent {
        family_id: FamilyId::from_uuid(id(10)),
        event_id: EventId::from_uuid(id(20)),
    };
    family(store, 10, 1001, link).await;
    assert_eq!(linking(store, RecordLink::FamilyEvent, &[20]).await, pairs(&[(10, 20)]));

    create_repository(store, 60).await;
    create_source(store, 40).await;
    hold(store, 40, 60, 4001).await;
    assert_eq!(
        linking(store, RecordLink::SourceRepository, &[60]).await,
        pairs(&[(40, 60)])
    );
}

async fn a_dropped_reference_marks_its_target_for_rematching(store: &Store) {
    create_event(store, 20).await;
    create_person(store, 1).await;
    take_part(store, 1, 20, 101).await;
    let keys = store.match_dirty().await.unwrap();
    store.rekey_matches(&[], &keys).await.unwrap();
    let pairs = store.match_pairs_dirty().await.unwrap();
    store.refresh_match_pairs(&[], &pairs).await.unwrap();

    let retract = PersonCommand::RetractAssertion {
        person_id: PersonId::from_uuid(id(1)),
        target: AssertionId::from_uuid(id(101)),
    };
    person(store, 1, 102, retract).await;
    let event = id(20).to_string();
    for dirty in [
        store.match_dirty().await.unwrap(),
        store.match_pairs_dirty().await.unwrap(),
    ] {
        assert!(
            dirty.iter().any(|record| record.aggregate_id == event),
            "the event its participant left is marked: {dirty:?}"
        );
    }
}

async fn a_rebuild_reproduces_the_links(store: &Store) {
    create_event(store, 20).await;
    create_person(store, 1).await;
    take_part(store, 1, 20, 101).await;
    create_place(store, 30).await;
    create_place(store, 31).await;
    enclose(store, 30, 31, 3001).await;
    create_repository(store, 60).await;
    create_source(store, 40).await;
    hold(store, 40, 60, 4001).await;
    store.rebuild_projections().await.unwrap();
    assert_eq!(
        linking(store, RecordLink::Participation, &[20]).await,
        pairs(&[(1, 20)])
    );
    assert_eq!(
        linking(store, RecordLink::PlaceEnclosure, &[31]).await,
        pairs(&[(30, 31)])
    );
    assert_eq!(
        linking(store, RecordLink::SourceRepository, &[60]).await,
        pairs(&[(40, 60)])
    );
}

#[cfg(feature = "sqlite")]
mod sqlite {
    use vitni_db::{RecordLink, Store};

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
        views_are_read_by_id_skipping_unknown_ones,
        more_ids_than_one_query_binds_are_all_read,
        a_familys_partners_and_children_link_to_it_while_they_are_members,
        a_persons_participations_link_it_to_their_events_until_retracted,
        an_event_links_to_its_place_and_a_citation_to_its_source,
        a_place_links_to_its_enclosing_places_until_retracted,
        a_family_links_to_its_events_and_a_source_to_its_repositories,
        a_dropped_reference_marks_its_target_for_rematching,
        a_rebuild_reproduces_the_links,
    );

    #[tokio::test]
    async fn a_workspace_opened_without_the_index_fills_it_from_its_projections() {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("ws.sqlite3").display());
        let store = Store::open(&url).await.unwrap();
        super::create_event(&store, 20).await;
        super::create_person(&store, 1).await;
        super::take_part(&store, 1, 20, 101).await;
        drop(store);

        let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
        sqlx::query("DROP TABLE record_links").execute(&pool).await.unwrap();
        pool.close().await;

        let store = Store::open(&url).await.unwrap();
        assert_eq!(
            super::linking(&store, RecordLink::Participation, &[20]).await,
            super::pairs(&[(1, 20)])
        );
    }
}

#[cfg(feature = "postgres")]
mod postgres {
    use sqlx::migrate::Migrator;
    use test_containers_util::sqlx_pg::PostgresTestDb;
    use vitni_db::Store;

    static MIGRATIONS: Migrator = sqlx::migrate!();

    async fn store() -> (Store, PostgresTestDb) {
        let db = PostgresTestDb::create("vitni-pg", &MIGRATIONS, None, None).await;
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
        views_are_read_by_id_skipping_unknown_ones,
        more_ids_than_one_query_binds_are_all_read,
        a_familys_partners_and_children_link_to_it_while_they_are_members,
        a_persons_participations_link_it_to_their_events_until_retracted,
        an_event_links_to_its_place_and_a_citation_to_its_source,
        a_place_links_to_its_enclosing_places_until_retracted,
        a_family_links_to_its_events_and_a_source_to_its_repositories,
        a_dropped_reference_marks_its_target_for_rematching,
        a_rebuild_reproduces_the_links,
    );
}
