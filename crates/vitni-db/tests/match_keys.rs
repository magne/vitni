//! Integration tests for the `match_keys` blocking index (ADR 0038 §7), through the public `Store`
//! API, on SQLite and (under `--features postgres`, with a Docker daemon) Postgres: every test body is
//! one `async fn` over a `&Store`, run once per engine.

#![cfg(any(feature = "sqlite", feature = "postgres"))]
#![expect(clippy::unwrap_used, reason = "tests abort on setup/assertion failure")]

use time::macros::datetime;
use uuid::Uuid;
use vitni_core::enums::{EvidenceLevel, Sex};
use vitni_core::ids::{AgentId, AssertionId, HumanId, ImportRunId, PersonId};
use vitni_core::import_run::command::{ImportRunCommand, ImportRunCommandEnvelope, NewImportRun};
use vitni_core::matching::{MatchableKind, Probe};
use vitni_core::origin::DatasetId;
use vitni_core::person::command::{PersonCommand, PersonCommandEnvelope};
use vitni_core::provenance::{Agent, AgentKind, AssertionMeta, EventContext, Timestamp};
use vitni_db::{DirtyRecord, KeyedRecord, Store};

fn meta(assertion: u128) -> AssertionMeta {
    AssertionMeta {
        assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
        context: EventContext {
            operator: Agent {
                kind: AgentKind::Human,
                id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                display: None,
            },
            occurred_at: Timestamp::new(datetime!(2026-09-30 12:00:00 UTC)),
            rationale: None,
            confidence: None,
            citations: Vec::new(),
            evidence_analysis: None,
            origin: None,
        },
    }
}

fn person_id(n: u128) -> PersonId {
    PersonId::from_uuid(Uuid::from_u128(n))
}

async fn person(store: &Store, n: u128, assertion: u128, command: PersonCommand) {
    store
        .execute_person(
            &person_id(n).to_string(),
            PersonCommandEnvelope {
                meta: meta(assertion),
                command,
            },
        )
        .await
        .unwrap();
}

async fn create_person(store: &Store, n: u128) {
    let command = PersonCommand::CreatePerson {
        person_id: person_id(n),
        human_id: HumanId::new(format!("I{n:04}")),
        evidence_level: EvidenceLevel::Conclusion,
        external_ids: Vec::new(),
    };
    person(store, n, n * 100, command).await;
}

async fn assert_sex(store: &Store, n: u128, assertion: u128) {
    let command = PersonCommand::AssertSex {
        person_id: person_id(n),
        sex: Sex::Female,
    };
    person(store, n, assertion, command).await;
}

fn keyed(n: u128, keys: &[&str]) -> KeyedRecord {
    KeyedRecord {
        kind: MatchableKind::Person,
        aggregate_id: person_id(n).to_string(),
        keys: keys.iter().map(|key| (*key).to_owned()).collect(),
    }
}

async fn a_commit_marks_its_record_dirty_and_bumps_the_generation(store: &Store) {
    create_person(store, 1).await;
    let dirty = store.match_dirty().await.unwrap();
    assert_eq!(
        dirty,
        [DirtyRecord {
            kind: MatchableKind::Person,
            aggregate_id: person_id(1).to_string(),
            generation: 1,
        }]
    );
    assert_sex(store, 1, 101).await;
    assert_eq!(store.match_dirty().await.unwrap()[0].generation, 2);
}

async fn an_unmatched_kind_is_never_dirty(store: &Store) {
    let run_id = ImportRunId::from_uuid(Uuid::from_u128(5));
    let start = ImportRunCommand::StartImportRun {
        run_id,
        run: NewImportRun {
            plugin: "gedcom-import".to_owned(),
            plugin_version: "0.1.0".to_owned(),
            dataset: DatasetId::global("gedcom"),
            dataset_label: "tree.ged".to_owned(),
            source_label: "tree.ged".to_owned(),
            file_asserted_at: None,
        },
    };
    store
        .execute_import_run(
            &run_id.to_string(),
            ImportRunCommandEnvelope {
                meta: meta(1),
                command: start,
            },
        )
        .await
        .unwrap();
    assert!(store.match_dirty().await.unwrap().is_empty());
}

async fn a_rekey_clears_only_the_generation_it_read(store: &Store) {
    create_person(store, 1).await;
    create_person(store, 2).await;
    let read = store.match_dirty().await.unwrap();
    assert_sex(store, 1, 101).await;
    store
        .rekey_matches(&[keyed(1, &["t:ole@185"]), keyed(2, &["t:kari@185"])], &read)
        .await
        .unwrap();
    let dirty = store.match_dirty().await.unwrap();
    assert_eq!(dirty.len(), 1, "the person touched meanwhile stays dirty: {dirty:?}");
    assert_eq!(dirty[0].aggregate_id, person_id(1).to_string());
    assert!(dirty[0].generation > read[0].generation);
}

async fn a_rekey_that_lost_a_race_marks_its_records_dirty_again(store: &Store) {
    create_person(store, 1).await;
    let slow = store.match_dirty().await.unwrap();
    assert_sex(store, 1, 101).await;
    let fast = store.match_dirty().await.unwrap();
    store.rekey_matches(&[keyed(1, &["t:new@185"])], &fast).await.unwrap();
    assert!(
        store.match_dirty().await.unwrap().is_empty(),
        "the fresh rekey cleared it"
    );
    store.rekey_matches(&[keyed(1, &["t:old@185"])], &slow).await.unwrap();
    let dirty = store.match_dirty().await.unwrap();
    assert_eq!(
        dirty.len(),
        1,
        "the stale keys are rekeyed on the next lookup: {dirty:?}"
    );
    assert_eq!(dirty[0].aggregate_id, person_id(1).to_string());
}

async fn a_rebuild_that_another_rebuild_beat_writes_nothing(store: &Store) {
    create_person(store, 1).await;
    let read = store.match_dirty().await.unwrap();
    store
        .reset_match_keys("f1", &[keyed(1, &["t:new@?"])], &read)
        .await
        .unwrap();
    store
        .reset_match_keys("f1", &[keyed(1, &["t:old@?"])], &read)
        .await
        .unwrap();
    let keys = store.match_keys_of_kind(MatchableKind::Person).await.unwrap();
    assert_eq!(keys, [(person_id(1).to_string(), "t:new@?".to_owned())]);
}

async fn a_rekey_replaces_a_records_keys(store: &Store) {
    store
        .reset_match_keys(
            "f1",
            &[keyed(1, &["t:ole@185", "p:ol@185"]), keyed(2, &["t:ole@185"])],
            &[],
        )
        .await
        .unwrap();
    store
        .rekey_matches(&[keyed(1, &["t:per@185"]), keyed(2, &[])], &[])
        .await
        .unwrap();
    let keys = store.match_keys_of_kind(MatchableKind::Person).await.unwrap();
    assert_eq!(keys, [(person_id(1).to_string(), "t:per@185".to_owned())]);
}

async fn candidates_meet_exact_keys_and_prefixes_within_their_kind(store: &Store) {
    let place = KeyedRecord {
        kind: MatchableKind::Place,
        aggregate_id: "place-1".to_owned(),
        keys: vec!["t:ole@185".to_owned()],
    };
    let records = [
        keyed(1, &["t:ole@185"]),
        keyed(2, &["t:ole@?"]),
        keyed(3, &["t:ole@190"]),
        keyed(4, &["t:olea@185"]),
        place,
    ];
    store.reset_match_keys("f1", &records, &[]).await.unwrap();
    let dated = Probe::of(&["t:ole@186".to_owned()]);
    let found = store.match_candidates(MatchableKind::Person, &dated).await.unwrap();
    assert_eq!(found, [person_id(1).to_string(), person_id(2).to_string()]);
    let undated = Probe::of(&["t:ole@?".to_owned()]);
    let found = store.match_candidates(MatchableKind::Person, &undated).await.unwrap();
    assert_eq!(
        found,
        [
            person_id(1).to_string(),
            person_id(2).to_string(),
            person_id(3).to_string()
        ],
        "an unknown decade meets every decade, and only of the same base"
    );
    let nothing = store
        .match_candidates(MatchableKind::Person, &Probe::default())
        .await
        .unwrap();
    assert!(nothing.is_empty());
}

async fn a_reset_records_its_fingerprint_and_a_rebuild_forgets_it(store: &Store) {
    assert_eq!(store.match_keys_fingerprint().await.unwrap(), None);
    create_person(store, 1).await;
    let read = store.match_dirty().await.unwrap();
    store
        .reset_match_keys("f1", &[keyed(1, &["t:ole@?"])], &read)
        .await
        .unwrap();
    assert_eq!(store.match_keys_fingerprint().await.unwrap().as_deref(), Some("f1"));
    assert!(store.match_dirty().await.unwrap().is_empty());
    store
        .reset_match_keys("f2", &[keyed(1, &["t:ole@?"])], &[])
        .await
        .unwrap();
    assert_eq!(store.match_keys_fingerprint().await.unwrap().as_deref(), Some("f2"));
    store.rebuild_projections().await.unwrap();
    assert_eq!(store.match_keys_fingerprint().await.unwrap(), None);
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
        a_commit_marks_its_record_dirty_and_bumps_the_generation,
        a_rekey_that_lost_a_race_marks_its_records_dirty_again,
        a_rebuild_that_another_rebuild_beat_writes_nothing,
        an_unmatched_kind_is_never_dirty,
        a_rekey_clears_only_the_generation_it_read,
        a_rekey_replaces_a_records_keys,
        candidates_meet_exact_keys_and_prefixes_within_their_kind,
        a_reset_records_its_fingerprint_and_a_rebuild_forgets_it,
    );
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
        a_commit_marks_its_record_dirty_and_bumps_the_generation,
        a_rekey_that_lost_a_race_marks_its_records_dirty_again,
        a_rebuild_that_another_rebuild_beat_writes_nothing,
        an_unmatched_kind_is_never_dirty,
        a_rekey_clears_only_the_generation_it_read,
        a_rekey_replaces_a_records_keys,
        candidates_meet_exact_keys_and_prefixes_within_their_kind,
        a_reset_records_its_fingerprint_and_a_rebuild_forgets_it,
    );
}
