//! Integration tests for the `match_pairs` projection (ADR 0048), through the public `Store` API, on
//! SQLite and (under `--features postgres`, with a Docker daemon) Postgres: every test body is one
//! `async fn` over a `&Store`, run once per engine.

#![cfg(any(feature = "sqlite", feature = "postgres"))]
#![expect(clippy::unwrap_used, reason = "tests abort on setup/assertion failure")]

use time::macros::datetime;
use uuid::Uuid;
use vitni_core::enums::{EvidenceLevel, Sex};
use vitni_core::ids::{AgentId, AssertionId, HumanId, PersonId};
use vitni_core::matching::{MatchBand, MatchableKind};
use vitni_core::person::command::{PersonCommand, PersonCommandEnvelope};
use vitni_core::provenance::{Agent, AgentKind, AssertionMeta, EventContext, Timestamp};
use vitni_db::{MatchPair, PairRefresh, PairScope, Store};

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

fn person_id(n: u128) -> PersonId {
    PersonId::from_uuid(Uuid::from_u128(n))
}

fn id(n: u128) -> String {
    person_id(n).to_string()
}

async fn person(store: &Store, n: u128, assertion: u128, command: PersonCommand) {
    let envelope = PersonCommandEnvelope {
        meta: meta(assertion),
        command,
    };
    store.execute_person(&id(n), envelope).await.unwrap();
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

fn pair(a: u128, b: u128, band: MatchBand, score: f64) -> MatchPair {
    MatchPair {
        kind: MatchableKind::Person,
        a: id(a),
        b: id(b),
        band,
        score,
    }
}

/// The pairs read back, as `(a, b)` numbers.
async fn listed(store: &Store, min_band: MatchBand, limit: Option<usize>) -> Vec<(String, String)> {
    let pairs = store
        .match_pairs(&[MatchableKind::Person], min_band, limit)
        .await
        .unwrap();
    pairs.into_iter().map(|pair| (pair.a, pair.b)).collect()
}

async fn a_commit_marks_its_record_dirty_for_the_pairs_apart_from_the_keys(store: &Store) {
    create_person(store, 1).await;
    let read = store.match_dirty().await.unwrap();
    store.rekey_matches(&[], &read).await.unwrap();
    assert_eq!(store.match_dirty().await.unwrap(), []);
    let dirty = store.match_pairs_dirty().await.unwrap();
    assert_eq!(dirty.len(), 1, "the pairs have their own dirty set: {dirty:?}");
    assert_eq!(dirty[0].aggregate_id, id(1));
}

async fn pairs_are_read_strongest_first_from_a_band_up(store: &Store) {
    let pairs = [
        pair(1, 2, MatchBand::Possible, 0.4),
        pair(1, 3, MatchBand::Probable, 0.8),
        pair(2, 3, MatchBand::Probable, 0.9),
    ];
    store.reset_match_pairs("f1", &pairs, &[]).await.unwrap();
    assert_eq!(
        listed(store, MatchBand::Possible, None).await,
        [(id(2), id(3)), (id(1), id(3)), (id(1), id(2))]
    );
    assert_eq!(listed(store, MatchBand::Probable, Some(1)).await, [(id(2), id(3))]);
    assert_eq!(
        store.match_pair_counts(MatchBand::Possible).await.unwrap(),
        [(MatchableKind::Person, 3)]
    );
    assert_eq!(
        store
            .match_pairs(&[MatchableKind::Place], MatchBand::Possible, None)
            .await
            .unwrap(),
        []
    );
    let read = store
        .match_pairs(&MatchableKind::ALL, MatchBand::Possible, Some(1))
        .await
        .unwrap();
    assert_eq!(
        read,
        [pair(2, 3, MatchBand::Probable, 0.9)],
        "a read keeps band and score"
    );
}

async fn a_refresh_replaces_only_the_pairs_of_its_records(store: &Store) {
    let pairs = [
        pair(1, 2, MatchBand::Possible, 0.4),
        pair(2, 3, MatchBand::Possible, 0.5),
        pair(3, 4, MatchBand::Possible, 0.6),
    ];
    store.reset_match_pairs("f1", &pairs, &[]).await.unwrap();
    let refresh = PairRefresh {
        kind: MatchableKind::Person,
        scope: PairScope::Records(vec![id(2)]),
        pairs: vec![pair(2, 4, MatchBand::Probable, 0.7)],
    };
    store.refresh_match_pairs(&[refresh], &[]).await.unwrap();
    assert_eq!(
        listed(store, MatchBand::Possible, None).await,
        [(id(2), id(4)), (id(3), id(4))],
        "both pairs naming the record went, the other stayed"
    );
    let whole = PairRefresh {
        kind: MatchableKind::Person,
        scope: PairScope::Kind,
        pairs: vec![pair(1, 4, MatchBand::Possible, 0.3)],
    };
    store.refresh_match_pairs(&[whole], &[]).await.unwrap();
    assert_eq!(listed(store, MatchBand::Possible, None).await, [(id(1), id(4))]);
}

async fn a_refresh_clears_only_the_generation_it_read(store: &Store) {
    create_person(store, 1).await;
    create_person(store, 2).await;
    let read = store.match_pairs_dirty().await.unwrap();
    assert_sex(store, 1, 101).await;
    store.refresh_match_pairs(&[], &read).await.unwrap();
    let dirty = store.match_pairs_dirty().await.unwrap();
    assert_eq!(dirty.len(), 1, "the person touched meanwhile stays dirty: {dirty:?}");
    assert_eq!(dirty[0].aggregate_id, id(1));
}

async fn a_reset_records_its_fingerprint_unless_beaten_and_a_rebuild_forgets_it(store: &Store) {
    assert_eq!(store.match_pairs_fingerprint().await.unwrap(), None);
    create_person(store, 1).await;
    let read = store.match_pairs_dirty().await.unwrap();
    let first = [pair(1, 2, MatchBand::Possible, 0.4)];
    store.reset_match_pairs("f1", &first, &read).await.unwrap();
    assert_eq!(store.match_pairs_fingerprint().await.unwrap().as_deref(), Some("f1"));
    assert_eq!(store.match_pairs_dirty().await.unwrap(), []);
    let late = [pair(1, 3, MatchBand::Possible, 0.4)];
    store.reset_match_pairs("f1", &late, &[]).await.unwrap();
    assert_eq!(
        listed(store, MatchBand::Possible, None).await,
        [(id(1), id(2))],
        "a rebuild another one beat writes nothing"
    );
    store.rebuild_projections().await.unwrap();
    assert_eq!(store.match_pairs_fingerprint().await.unwrap(), None);
}

async fn a_decided_pair_is_left_out(store: &Store) {
    for n in 1..=4 {
        create_person(store, n).await;
    }
    let pairs = [
        pair(1, 2, MatchBand::Possible, 0.4),
        pair(1, 3, MatchBand::Possible, 0.5),
        pair(3, 4, MatchBand::Possible, 0.6),
    ];
    store.reset_match_pairs("f1", &pairs, &[]).await.unwrap();
    let merge = PersonCommand::MergePersons {
        surviving: person_id(4),
        merged: person_id(2),
        assessment: None,
    };
    person(store, 4, 401, merge).await;
    let distinguish = PersonCommand::DistinguishPersons {
        person: person_id(2),
        other: person_id(3),
        assessment: None,
    };
    person(store, 2, 201, distinguish).await;
    assert_eq!(
        listed(store, MatchBand::Possible, None).await,
        [(id(1), id(3))],
        "a merged member's pair and the pair of two clusters held distinct are left out"
    );
    assert_eq!(
        store.match_pair_counts(MatchBand::Possible).await.unwrap(),
        [(MatchableKind::Person, 1)]
    );
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
        a_commit_marks_its_record_dirty_for_the_pairs_apart_from_the_keys,
        pairs_are_read_strongest_first_from_a_band_up,
        a_refresh_replaces_only_the_pairs_of_its_records,
        a_refresh_clears_only_the_generation_it_read,
        a_reset_records_its_fingerprint_unless_beaten_and_a_rebuild_forgets_it,
        a_decided_pair_is_left_out,
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
        a_commit_marks_its_record_dirty_for_the_pairs_apart_from_the_keys,
        pairs_are_read_strongest_first_from_a_band_up,
        a_refresh_replaces_only_the_pairs_of_its_records,
        a_refresh_clears_only_the_generation_it_read,
        a_reset_records_its_fingerprint_unless_beaten_and_a_rebuild_forgets_it,
        a_decided_pair_is_left_out,
    );
}
