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

/// The state a write killed after its events committed and before its index rows leaves: the write
/// journal still names the aggregate, and the index holds none of its rows.
const KILL_BEFORE_INDEX: [&str; 2] = [
    "INSERT INTO record_origins_pending (aggregate_type, aggregate_id) \
     VALUES ('person', '00000000-0000-0000-0000-000000000001')",
    "DELETE FROM record_origins",
];

/// The state a write killed after its index rows and before its journal entry was withdrawn leaves.
const KILL_BEFORE_JOURNAL_END: [&str; 1] = [KILL_BEFORE_INDEX[0]];

/// How many writes the journal says may still lack their index rows.
const PENDING_WRITES: &str = "SELECT COUNT(*) FROM record_origins_pending";

/// Writes [`seed_two_origins`]' events while every index insert fails, and checks none was indexed.
async fn seed_while_the_index_fails(store: &Store) {
    create_person(store).await;
    person(store, 11, Some(origin("I1", None)), assert_sex(Sex::Female)).await;
    assert_eq!(store.record_origins_dump().await.unwrap(), Vec::<Vec<String>>::new());
}

/// After the failing index is fixed and the workspace reopened, both writes are indexed.
async fn the_failed_writes_are_indexed(store: &Store) {
    assert_eq!(store.record_origins_dump().await.unwrap().len(), 2, "created, sex");
    let resolution = store
        .resolve_origin(dataset().as_str(), "I1", None, "person")
        .await
        .unwrap();
    assert!(resolution.is_some(), "the record resolves onto the person");
}

/// After a reopen, the index is the one the writes made, and a re-run of the import resolves the
/// record onto the person it made instead of creating it again.
async fn the_index_is_whole(store: &Store, before: &[Vec<String>]) {
    assert_eq!(store.record_origins_dump().await.unwrap(), before);
    let resolution = store
        .resolve_origin(dataset().as_str(), "I1", None, "person")
        .await
        .unwrap();
    assert!(resolution.is_some(), "the record resolves onto the person");
}

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

    /// A seeded workspace closed and then changed by `statements`, its URL, the index the seeding
    /// made, and how many journal entries the seeding left behind.
    async fn damaged_workspace(statements: &[&str]) -> (tempfile::TempDir, String, Vec<Vec<String>>, i64) {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("ws.sqlite3").display());
        let store = Store::open(&url).await.unwrap();
        let before = super::seed_two_origins(&store).await;
        drop(store);

        let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
        let pending: i64 = sqlx::query_scalar(super::PENDING_WRITES)
            .fetch_one(&pool)
            .await
            .unwrap();
        for statement in statements {
            sqlx::query(statement).execute(&pool).await.unwrap();
        }
        pool.close().await;
        (dir, url, before, pending)
    }

    async fn interrupted_workspace() -> (tempfile::TempDir, String, Vec<Vec<String>>) {
        let (dir, url, before, _) = damaged_workspace(&super::INTERRUPT_BACKFILL).await;
        (dir, url, before)
    }

    #[tokio::test]
    async fn a_finished_write_leaves_no_journal_entry() {
        let (_dir, _url, _before, pending) = damaged_workspace(&[]).await;
        assert_eq!(pending, 0, "every open would replay a write that finished");
    }

    #[tokio::test]
    async fn a_write_killed_before_its_index_rows_is_indexed_on_the_next_open() {
        let (_dir, url, before, _) = damaged_workspace(&super::KILL_BEFORE_INDEX).await;
        let store = Store::open(&url).await.unwrap();
        super::the_index_is_whole(&store, &before).await;
    }

    #[tokio::test]
    async fn a_journalled_write_already_indexed_is_not_indexed_twice() {
        let (_dir, url, before, _) = damaged_workspace(&super::KILL_BEFORE_JOURNAL_END).await;
        let store = Store::open(&url).await.unwrap();
        super::the_index_is_whole(&store, &before).await;
    }

    #[tokio::test]
    async fn a_failed_index_write_is_indexed_on_the_next_open() {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("ws.sqlite3").display());
        let store = Store::open(&url).await.unwrap();
        let pool = sqlx::SqlitePool::connect(&url).await.unwrap();
        sqlx::query(
            "CREATE TRIGGER fail_index BEFORE INSERT ON record_origins BEGIN SELECT RAISE(ABORT, 'disk full'); END",
        )
        .execute(&pool)
        .await
        .unwrap();
        super::seed_while_the_index_fails(&store).await;
        drop(store);
        sqlx::query("DROP TRIGGER fail_index").execute(&pool).await.unwrap();
        pool.close().await;

        let store = Store::open(&url).await.unwrap();
        super::the_failed_writes_are_indexed(&store).await;
    }

    #[tokio::test]
    async fn an_index_without_event_keys_is_rebuilt() {
        let without_keys = [
            "DROP INDEX record_origins_event",
            "ALTER TABLE record_origins DROP COLUMN event_key",
        ];
        let (_dir, url, before, _) = damaged_workspace(&without_keys).await;
        let store = Store::open(&url).await.unwrap();
        super::the_index_is_whole(&store, &before).await;
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

    /// A seeded database closed and then changed by `statements`, the index the seeding made, and
    /// how many journal entries the seeding left behind.
    async fn damaged_database(statements: &[&str]) -> (PostgresTestDb, Vec<Vec<String>>, i64) {
        let (store, db) = store().await;
        let before = super::seed_two_origins(&store).await;
        drop(store);

        let pool = sqlx::PgPool::connect(db.dsn()).await.unwrap();
        let pending: i64 = sqlx::query_scalar(super::PENDING_WRITES)
            .fetch_one(&pool)
            .await
            .unwrap();
        for statement in statements {
            sqlx::query(statement).execute(&pool).await.unwrap();
        }
        pool.close().await;
        (db, before, pending)
    }

    async fn interrupted_database() -> (PostgresTestDb, Vec<Vec<String>>) {
        let (db, before, _) = damaged_database(&super::INTERRUPT_BACKFILL).await;
        (db, before)
    }

    #[tokio::test]
    async fn a_finished_write_leaves_no_journal_entry() {
        let (_db, _before, pending) = damaged_database(&[]).await;
        assert_eq!(pending, 0, "every open would replay a write that finished");
    }

    #[tokio::test]
    async fn a_write_killed_before_its_index_rows_is_indexed_on_the_next_open() {
        let (db, before, _) = damaged_database(&super::KILL_BEFORE_INDEX).await;
        let store = Store::open(db.dsn()).await.unwrap();
        super::the_index_is_whole(&store, &before).await;
    }

    #[tokio::test]
    async fn a_journalled_write_already_indexed_is_not_indexed_twice() {
        let (db, before, _) = damaged_database(&super::KILL_BEFORE_JOURNAL_END).await;
        let store = Store::open(db.dsn()).await.unwrap();
        super::the_index_is_whole(&store, &before).await;
    }

    #[tokio::test]
    async fn a_failed_index_write_is_indexed_on_the_next_open() {
        let (store, db) = store().await;
        let pool = sqlx::PgPool::connect(db.dsn()).await.unwrap();
        for statement in [
            "CREATE FUNCTION fail_index() RETURNS trigger AS $$ BEGIN RAISE EXCEPTION 'disk full'; END $$ LANGUAGE plpgsql",
            "CREATE TRIGGER fail_index BEFORE INSERT ON record_origins FOR EACH ROW EXECUTE FUNCTION fail_index()",
        ] {
            sqlx::query(statement).execute(&pool).await.unwrap();
        }
        super::seed_while_the_index_fails(&store).await;
        drop(store);
        sqlx::query("DROP TRIGGER fail_index ON record_origins")
            .execute(&pool)
            .await
            .unwrap();
        pool.close().await;

        let store = Store::open(db.dsn()).await.unwrap();
        super::the_failed_writes_are_indexed(&store).await;
    }

    #[tokio::test]
    async fn an_index_without_event_keys_is_rebuilt() {
        let (db, before, _) = damaged_database(&["ALTER TABLE record_origins DROP COLUMN event_key"]).await;
        let store = Store::open(db.dsn()).await.unwrap();
        super::the_index_is_whole(&store, &before).await;
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
