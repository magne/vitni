//! Integration tests for the `record_origins` index (ADR 0037 §4), through the public `Store` API,
//! on SQLite and (under `--features postgres`, with a Docker daemon) Postgres: every test body is
//! one `async fn` over a `&Store`, run once per engine.

#![cfg(any(feature = "sqlite", feature = "postgres"))]
#![expect(clippy::unwrap_used, reason = "tests abort on setup/assertion failure")]

use time::macros::datetime;
use uuid::Uuid;
use vitni_core::enums::{EvidenceLevel, Sex};
use vitni_core::ids::{AgentId, AssertionId, HumanId, ImportRunId, PersonId};
use vitni_core::import_run::command::{ImportRunCommand, ImportRunCommandEnvelope, NewImportRun};
use vitni_core::import_run::event::{ImportCounts, ResolutionDecision, ResolvedItem};
use vitni_core::origin::{DatasetId, RecordOrigin};
use vitni_core::person::command::{PersonCommand, PersonCommandEnvelope};
use vitni_core::provenance::{Agent, AgentKind, AssertionMeta, EventContext, Timestamp};
use vitni_db::Store;

const PERSON: u128 = 1;
const RUN: u128 = 0x5;

fn dataset() -> DatasetId {
    DatasetId::lineage("gedcom", Uuid::from_u128(0xD))
}

fn origin(record: &str, item: Option<&str>) -> RecordOrigin {
    RecordOrigin {
        dataset: dataset(),
        record: record.to_owned(),
        item: item.map(str::to_owned),
        digest: None,
        run: ImportRunId::from_uuid(Uuid::from_u128(RUN)),
    }
}

fn meta(assertion: u128, origin: Option<RecordOrigin>) -> AssertionMeta {
    AssertionMeta {
        assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
        context: EventContext {
            operator: Agent {
                kind: AgentKind::Human,
                id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                display: None,
            },
            occurred_at: Timestamp::new(datetime!(2026-09-29 12:00:00 UTC)),
            rationale: None,
            confidence: None,
            citations: Vec::new(),
            evidence_analysis: None,
            origin: origin.map(Box::new),
        },
    }
}

fn person_id() -> PersonId {
    PersonId::from_uuid(Uuid::from_u128(PERSON))
}

async fn person(store: &Store, assertion: u128, origin: Option<RecordOrigin>, command: PersonCommand) {
    store
        .execute_person(
            &person_id().to_string(),
            PersonCommandEnvelope {
                meta: meta(assertion, origin),
                command,
            },
        )
        .await
        .unwrap();
}

async fn create_person(store: &Store) {
    let command = PersonCommand::CreatePerson {
        person_id: person_id(),
        human_id: HumanId::new("I0001"),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    person(store, 10, Some(origin("I1", None)), command).await;
}

fn assert_sex(sex: Sex) -> PersonCommand {
    PersonCommand::AssertSex {
        person_id: person_id(),
        sex,
    }
}

async fn an_originated_write_is_indexed_and_a_keyboard_write_is_not(store: &Store) {
    create_person(store).await;
    person(store, 11, Some(origin("I1", None)), assert_sex(Sex::Female)).await;
    person(store, 12, None, assert_sex(Sex::Male)).await;

    let rows = store
        .origin_rows(dataset().as_str(), "I1", None, "person.SexAsserted")
        .await
        .unwrap();
    assert_eq!(rows.len(), 1, "only the imported sex: {rows:?}");
    let row = &rows[0];
    assert_eq!(row.aggregate_kind, "person");
    assert_eq!(row.aggregate_id, person_id().to_string());
    assert_eq!(row.assertion_id, AssertionId::from_uuid(Uuid::from_u128(11)));
    assert!(row.live);
    let body = vitni_core::person::event::PersonEventBody::SexAsserted {
        person_id: person_id(),
        sex: Sex::Female,
    };
    assert_eq!(
        row.digest,
        vitni_db::digest(&[&body]).unwrap(),
        "the row digests its body"
    );
}

async fn an_item_resolves_onto_the_aggregate_its_creating_event_made(store: &Store) {
    create_person(store).await;
    let resolved = store
        .resolve_origin(dataset().as_str(), "I1", None, "person")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resolved.aggregate_id, person_id().to_string());
    assert!(resolved.created, "the dataset created it");
    let other_item = store
        .resolve_origin(dataset().as_str(), "I1", Some("event:BIRT:0"), "person")
        .await
        .unwrap();
    assert_eq!(other_item, None, "another item of the same record is another entity");
    let other_kind = store
        .resolve_origin(dataset().as_str(), "I1", None, "family")
        .await
        .unwrap();
    assert_eq!(other_kind, None);
    let other_dataset = store
        .resolve_origin("gedcom:other", "I1", None, "person")
        .await
        .unwrap();
    assert_eq!(other_dataset, None, "datasets scope record ids");
}

async fn the_creating_origins_of_a_kind_are_read_at_once(store: &Store) {
    create_person(store).await;
    person(store, 11, Some(origin("I1", Some("x"))), assert_sex(Sex::Female)).await;
    let created = store.created_origins("person").await.unwrap();
    assert_eq!(created, [(person_id().to_string(), origin("I1", None))]);
    let checked = store.created_origins("family").await.unwrap();
    assert!(checked.is_empty(), "{checked:?}");
}

async fn one_aggregates_creating_origin_is_read_alone(store: &Store) {
    create_person(store).await;
    person(store, 11, Some(origin("I1", Some("x"))), assert_sex(Sex::Female)).await;
    let created = store.created_origin("person", &person_id().to_string()).await.unwrap();
    assert_eq!(created, Some(origin("I1", None)));
    let other = PersonId::from_uuid(Uuid::from_u128(0x99)).to_string();
    assert_eq!(store.created_origin("person", &other).await.unwrap(), None);
    assert_eq!(
        store.created_origin("family", &person_id().to_string()).await.unwrap(),
        None,
        "another kind's aggregate"
    );
}

async fn the_live_origin_of_each_assertion_on_an_aggregate_is_read(store: &Store) {
    create_person(store).await;
    person(store, 11, Some(origin("I1", Some("x"))), assert_sex(Sex::Female)).await;
    person(store, 12, None, assert_sex(Sex::Male)).await;
    person(store, 13, Some(origin("I2", None)), assert_sex(Sex::Unknown)).await;
    let retract = PersonCommand::RetractAssertion {
        person_id: person_id(),
        target: AssertionId::from_uuid(Uuid::from_u128(13)),
    };
    person(store, 14, None, retract).await;

    let origins = store
        .assertion_origins("person", &person_id().to_string())
        .await
        .unwrap();
    let assertion = |n: u128| AssertionId::from_uuid(Uuid::from_u128(n));
    assert_eq!(
        origins,
        [
            (assertion(10), origin("I1", None)),
            (assertion(11), origin("I1", Some("x")))
        ],
        "the keyboard write has no origin and the retracted one is not live"
    );
    let other = PersonId::from_uuid(Uuid::from_u128(0x99)).to_string();
    let none = store.assertion_origins("person", &other).await.unwrap();
    assert!(none.is_empty(), "{none:?}");
}

async fn a_retraction_clears_the_rows_live_flag(store: &Store) {
    create_person(store).await;
    person(store, 11, Some(origin("I1", None)), assert_sex(Sex::Female)).await;
    let retract = PersonCommand::RetractAssertion {
        person_id: person_id(),
        target: AssertionId::from_uuid(Uuid::from_u128(11)),
    };
    person(store, 12, None, retract).await;

    let rows = store
        .origin_rows(dataset().as_str(), "I1", None, "person.SexAsserted")
        .await
        .unwrap();
    assert_eq!(rows.len(), 1);
    assert!(!rows[0].live, "the retracted assertion is no longer live");
}

async fn run(store: &Store, assertion: u128, command: ImportRunCommand) {
    store
        .execute_import_run(
            &ImportRunId::from_uuid(Uuid::from_u128(RUN)).to_string(),
            ImportRunCommandEnvelope {
                meta: meta(assertion, None),
                command,
            },
        )
        .await
        .unwrap();
}

async fn a_recorded_resolution_resolves_the_item_in_later_runs(store: &Store) {
    let run_id = ImportRunId::from_uuid(Uuid::from_u128(RUN));
    let start = ImportRunCommand::StartImportRun {
        run_id,
        run: NewImportRun {
            plugin: "gedcom-import".to_owned(),
            plugin_version: "0.1.0".to_owned(),
            dataset: dataset(),
            dataset_label: "tree.ged".to_owned(),
            source_label: "tree.ged".to_owned(),
            file_asserted_at: None,
            dataset_hint: None,
        },
    };
    run(store, 20, start).await;
    let resolved = ResolvedItem {
        record: "I7".to_owned(),
        item: None,
        kind: "person".to_owned(),
        aggregate_id: Uuid::from_u128(PERSON),
        decision: ResolutionDecision::ExternalId,
    };
    let finish = ImportRunCommand::FinishImportRun {
        run_id,
        resolved: vec![resolved],
        counts: ImportCounts::default(),
    };
    run(store, 21, finish).await;

    let resolved = store
        .resolve_origin(dataset().as_str(), "I7", None, "person")
        .await
        .unwrap()
        .unwrap();
    assert_eq!(resolved.aggregate_id, person_id().to_string());
    assert!(!resolved.created, "the dataset only resolved onto it");
}

async fn the_records_each_dataset_already_holds_are_counted(store: &Store) {
    create_person(store).await;
    a_recorded_resolution_resolves_the_item_in_later_runs(store).await;
    let other = RecordOrigin {
        dataset: DatasetId::lineage("gedcom", Uuid::from_u128(0xE)),
        ..origin("I1", None)
    };
    let command = PersonCommand::CreatePerson {
        person_id: PersonId::from_uuid(Uuid::from_u128(2)),
        human_id: HumanId::new("I0002"),
        evidence_level: EvidenceLevel::Persona,
        external_ids: Vec::new(),
    };
    store
        .execute_person(
            &PersonId::from_uuid(Uuid::from_u128(2)).to_string(),
            PersonCommandEnvelope {
                meta: meta(30, Some(other.clone())),
                command,
            },
        )
        .await
        .unwrap();
    person(
        store,
        31,
        Some(origin("I8", Some("event:BIRT:0"))),
        assert_sex(Sex::Female),
    )
    .await;

    let records = ["I1", "I7", "I8", "I9"].map(str::to_owned);
    let overlap = store.origin_overlap("person", &records).await.unwrap();
    assert_eq!(
        overlap,
        [(dataset(), 2), (other.dataset, 1)],
        "a created and a resolved record count, an item's assertion does not"
    );
    assert_eq!(store.origin_overlap("family", &records).await.unwrap(), []);
    assert_eq!(store.origin_overlap("person", &[]).await.unwrap(), []);
}

async fn a_rebuild_reproduces_the_index(store: &Store) {
    create_person(store).await;
    person(store, 11, Some(origin("I1", None)), assert_sex(Sex::Female)).await;
    person(store, 12, Some(origin("I1", Some("sex:2"))), assert_sex(Sex::Male)).await;
    let retract = PersonCommand::RetractAssertion {
        person_id: person_id(),
        target: AssertionId::from_uuid(Uuid::from_u128(11)),
    };
    person(store, 13, None, retract).await;
    let before = store.record_origins_dump().await.unwrap();
    assert_eq!(before.len(), 3, "created, two sexes: {before:?}");

    store.rebuild_projections().await.unwrap();
    assert_eq!(store.record_origins_dump().await.unwrap(), before);
}

async fn a_preview_returns_the_events_and_writes_nothing(store: &Store) {
    create_person(store).await;
    let before = store.event_count().await.unwrap();
    let envelope = PersonCommandEnvelope {
        meta: meta(11, Some(origin("I1", None))),
        command: assert_sex(Sex::Female),
    };
    let events = store.preview_person(&person_id().to_string(), envelope).await.unwrap();
    assert_eq!(events.len(), 1);
    assert_eq!(
        events[0].body,
        vitni_core::person::event::PersonEventBody::SexAsserted {
            person_id: person_id(),
            sex: Sex::Female,
        }
    );
    assert_eq!(store.event_count().await.unwrap(), before, "nothing committed");
    let rows = store
        .origin_rows(dataset().as_str(), "I1", None, "person.SexAsserted")
        .await
        .unwrap();
    assert!(rows.is_empty(), "nothing indexed");
}

/// Writes two indexed events and returns the index they make.
async fn seed_two_origins(store: &Store) -> Vec<Vec<String>> {
    create_person(store).await;
    person(store, 11, Some(origin("I1", None)), assert_sex(Sex::Female)).await;
    let rows = store.record_origins_dump().await.unwrap();
    assert_eq!(rows.len(), 2, "created, sex: {rows:?}");
    rows
}

/// The state an open killed mid-backfill leaves: the index table exists, holds only the rows the
/// replay got through, and was never marked complete.
const INTERRUPT_BACKFILL: [&str; 2] = [
    "DELETE FROM record_origins_state",
    "DELETE FROM record_origins WHERE id = (SELECT MAX(id) FROM record_origins)",
];

#[cfg(feature = "sqlite")]
mod sqlite {
    use vitni_db::Store;

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
        an_originated_write_is_indexed_and_a_keyboard_write_is_not,
        an_item_resolves_onto_the_aggregate_its_creating_event_made,
        a_retraction_clears_the_rows_live_flag,
        the_creating_origins_of_a_kind_are_read_at_once,
        one_aggregates_creating_origin_is_read_alone,
        the_live_origin_of_each_assertion_on_an_aggregate_is_read,
        a_recorded_resolution_resolves_the_item_in_later_runs,
        a_rebuild_reproduces_the_index,
        a_preview_returns_the_events_and_writes_nothing,
        the_records_each_dataset_already_holds_are_counted,
    );

    #[tokio::test]
    async fn a_workspace_opened_without_the_index_fills_it_from_its_log() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ws.sqlite3");
        let url = format!("sqlite://{}", path.display());
        let store = Store::open(&url).await.unwrap();
        super::create_person(&store).await;
        let before = store.record_origins_dump().await.unwrap();
        assert_eq!(before.len(), 1);
        drop(store);

        let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
        for table in ["record_origins", "record_origins_state"] {
            sqlx::query(&format!("DROP TABLE {table}"))
                .execute(&pool)
                .await
                .unwrap();
        }
        pool.close().await;

        let store = Store::open(&url).await.unwrap();
        assert_eq!(store.record_origins_dump().await.unwrap(), before);
    }

    /// A workspace whose backfill was cut short, its URL, and the index a full backfill makes.
    async fn interrupted_workspace() -> (tempfile::TempDir, String, Vec<Vec<String>>) {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("ws.sqlite3").display());
        let store = Store::open(&url).await.unwrap();
        let before = super::seed_two_origins(&store).await;
        drop(store);

        let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
        for statement in super::INTERRUPT_BACKFILL {
            sqlx::query(statement).execute(&pool).await.unwrap();
        }
        pool.close().await;
        (dir, url, before)
    }

    #[tokio::test]
    async fn an_interrupted_backfill_is_resumed_on_the_next_open() {
        let (_dir, url, before) = interrupted_workspace().await;
        let store = Store::open(&url).await.unwrap();
        assert_eq!(store.record_origins_dump().await.unwrap(), before);
    }

    #[tokio::test]
    async fn two_opens_resuming_one_backfill_index_each_origin_once() {
        let (_dir, url, before) = interrupted_workspace().await;
        let (first, second) = tokio::join!(Store::open(&url), Store::open(&url));
        let (first, second) = (first.unwrap(), second.unwrap());
        assert_eq!(first.record_origins_dump().await.unwrap(), before);
        assert_eq!(second.record_origins_dump().await.unwrap(), before);
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
        an_originated_write_is_indexed_and_a_keyboard_write_is_not,
        an_item_resolves_onto_the_aggregate_its_creating_event_made,
        a_retraction_clears_the_rows_live_flag,
        the_creating_origins_of_a_kind_are_read_at_once,
        one_aggregates_creating_origin_is_read_alone,
        the_live_origin_of_each_assertion_on_an_aggregate_is_read,
        a_recorded_resolution_resolves_the_item_in_later_runs,
        a_rebuild_reproduces_the_index,
        a_preview_returns_the_events_and_writes_nothing,
        the_records_each_dataset_already_holds_are_counted,
    );

    /// A database whose backfill was cut short, and the index a full backfill makes.
    async fn interrupted_database() -> (PostgresTestDb, Vec<Vec<String>>) {
        let (store, db) = store().await;
        let before = super::seed_two_origins(&store).await;
        drop(store);

        let pool = sqlx::PgPool::connect(db.dsn()).await.unwrap();
        for statement in super::INTERRUPT_BACKFILL {
            sqlx::query(statement).execute(&pool).await.unwrap();
        }
        pool.close().await;
        (db, before)
    }

    #[tokio::test]
    async fn an_interrupted_backfill_is_resumed_on_the_next_open() {
        let (db, before) = interrupted_database().await;
        let store = Store::open(db.dsn()).await.unwrap();
        assert_eq!(store.record_origins_dump().await.unwrap(), before);
    }

    #[tokio::test]
    async fn two_opens_resuming_one_backfill_index_each_origin_once() {
        let (db, before) = interrupted_database().await;
        let (first, second) = tokio::join!(Store::open(db.dsn()), Store::open(db.dsn()));
        let (first, second) = (first.unwrap(), second.unwrap());
        assert_eq!(first.record_origins_dump().await.unwrap(), before);
        assert_eq!(second.record_origins_dump().await.unwrap(), before);
    }
}
