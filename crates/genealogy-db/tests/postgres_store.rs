//! Integration tests for the Postgres backend of the engine-neutral [`Store`] (ADR 0002).
//!
//! These exercise the real Postgres wiring against a containerized server: `test-containers-util`
//! reuses one container per process (`genealogy-pg`) and gives each test a fresh, randomly-named
//! database (dropped when the `PostgresTestDb` guard falls out of scope), so tests stay isolated
//! while sharing one container. A running Docker daemon is required; the tests compile only under
//! `--features postgres`. They use only the public `Store` surface — no `sqlx`/`postgres-es`/
//! `cqrs-es` types — proving the abstraction holds for Postgres exactly as for SQLite.

#![cfg(feature = "postgres")]
#![expect(clippy::unwrap_used, reason = "tests abort on setup/assertion failure")]

use genealogy_core::citation::command::{CitationCommand, CitationCommandEnvelope};
use genealogy_core::enums::EvidenceLevel;
use genealogy_core::id_format::IdFormat;
use genealogy_core::ids::{AgentId, AssertionId, CitationId, HumanId, PersonId, SourceId};
use genealogy_core::name::{NameType, PersonName, Surname};
use genealogy_core::person::command::{PersonCommand, PersonCommandEnvelope};
use genealogy_core::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
use genealogy_db::{CommandError, Store};
use sqlx::migrate::Migrator;
use test_containers_util::sqlx_pg::PostgresTestDb;
use time::macros::datetime;
use uuid::Uuid;

/// The shared container name; one container is started per test process and reused across tests.
const CONTAINER: &str = "genealogy-pg";

/// An empty migrator — our schema is created by `Store::open`, not by sqlx migrations, but the test
/// helper requires one.
static MIGRATIONS: Migrator = sqlx::migrate!();

/// Opens a `Store` over a fresh, isolated Postgres database. The returned guard must be kept alive
/// for the database's lifetime; it is dropped (and the database deleted) at the end of the test.
async fn store() -> (Store, PostgresTestDb) {
    let db = PostgresTestDb::create(CONTAINER, &MIGRATIONS, None, None).await;
    let store = Store::open(db.dsn()).await.unwrap();
    (store, db)
}

fn person_format() -> IdFormat {
    IdFormat::parse("I%04d").unwrap()
}

fn meta(assertion: u128) -> AssertionMeta {
    AssertionMeta {
        assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
        context: EventContext {
            operator: Agent {
                kind: AgentKind::Human,
                id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                display: None,
            },
            occurred_at: Timestamp::new(datetime!(2026-06-18 12:00:00 UTC)),
            rationale: None,
            confidence: Confidence::Normal,
            citations: Vec::new(),
            evidence_analysis: None,
        },
    }
}

async fn create(store: &Store, n: u128, human_id: &str) {
    let person_id = PersonId::from_uuid(Uuid::from_u128(n));
    store
        .execute_person(
            &person_id.to_string(),
            PersonCommandEnvelope {
                meta: meta(n * 10),
                command: PersonCommand::CreatePerson {
                    person_id,
                    human_id: HumanId::new(human_id),
                    evidence_level: EvidenceLevel::Conclusion,
                },
            },
        )
        .await
        .unwrap();
}

async fn name(store: &Store, n: u128, given: &str, surname: &str) {
    let person_id = PersonId::from_uuid(Uuid::from_u128(n));
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
    store
        .execute_person(
            &person_id.to_string(),
            PersonCommandEnvelope {
                meta: meta(n * 10 + 1),
                command: PersonCommand::AssertName { person_id, name },
            },
        )
        .await
        .unwrap();
}

#[tokio::test(flavor = "multi_thread")]
async fn allocates_sequential_ids_and_survives_width_growth() {
    let (store, _db) = store().await;
    assert_eq!(store.next_person_human_id(&person_format()).await.unwrap(), "I0001");

    create(&store, 1, "I0001").await;
    create(&store, 2, "I0002").await;
    assert_eq!(store.next_person_human_id(&person_format()).await.unwrap(), "I0003");

    // Past the zero-pad width, numbering must keep counting numerically, not lexicographically.
    create(&store, 9999, "I9999").await;
    assert_eq!(store.next_person_human_id(&person_format()).await.unwrap(), "I10000");
}

#[tokio::test(flavor = "multi_thread")]
async fn find_and_list_reflect_the_projection() {
    let (store, _db) = store().await;
    create(&store, 2, "I0002").await;
    name(&store, 2, "Alan", "Turing").await;
    create(&store, 1, "I0001").await;
    name(&store, 1, "Ada", "Lovelace").await;

    let found = store.find_person("I0001").await.unwrap().expect("exists");
    assert_eq!(found.human_id().map(HumanId::as_str), Some("I0001"));
    assert_eq!(found.names()[0].given.as_deref(), Some("Ada"));

    assert!(store.find_person("I0404").await.unwrap().is_none());

    let ids = person_ids(&store).await;
    assert_eq!(ids, ["I0001", "I0002"]);
}

#[tokio::test(flavor = "multi_thread")]
async fn a_domain_rejection_is_distinct_from_an_infrastructure_error() {
    let (store, _db) = store().await;
    create(&store, 1, "I0001").await;

    let person_id = PersonId::from_uuid(Uuid::from_u128(1));
    let err = store
        .execute_person(
            &person_id.to_string(),
            PersonCommandEnvelope {
                meta: meta(99),
                command: PersonCommand::CreatePerson {
                    person_id,
                    human_id: HumanId::new("I0001"),
                    evidence_level: EvidenceLevel::Conclusion,
                },
            },
        )
        .await;
    assert!(
        matches!(err, Err(CommandError::Rejected(_))),
        "re-create is a domain rejection"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn a_citation_against_a_missing_source_is_rejected() {
    // Exercises the Postgres cross-aggregate resolver (the §9 aggregate tax): the cited source does
    // not exist in the read model, so the pure decide rejects the citation.
    let (store, _db) = store().await;
    let citation_id = CitationId::from_uuid(Uuid::from_u128(1));
    let err = store
        .execute_citation(
            &citation_id.to_string(),
            CitationCommandEnvelope {
                meta: meta(1000),
                command: CitationCommand::CreateCitation {
                    citation_id,
                    human_id: HumanId::new("C0001"),
                    source_id: SourceId::from_uuid(Uuid::from_u128(0xDEAD)),
                },
            },
        )
        .await;
    assert!(
        matches!(err, Err(CommandError::Rejected(_))),
        "a citation against a missing source is a domain rejection"
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn rebuild_reproduces_the_projection_from_the_log() {
    let (store, _db) = store().await;
    create(&store, 1, "I0001").await;
    name(&store, 1, "Ada", "Lovelace").await;
    create(&store, 2, "I0002").await;
    name(&store, 2, "Alan", "Turing").await;

    let before = person_ids(&store).await;
    assert_eq!(before, ["I0001", "I0002"]);

    // Drop and replay every projection from the (untouched) event log.
    store.rebuild_projections().await.unwrap();

    let after = person_ids(&store).await;
    assert_eq!(after, before, "rebuild reproduces the projection identically");
    let found = store.find_person("I0001").await.unwrap().expect("exists after rebuild");
    assert_eq!(found.names()[0].given.as_deref(), Some("Ada"));
}

/// The ordered `human_id`s of every person projection.
async fn person_ids(store: &Store) -> Vec<String> {
    store
        .list_persons()
        .await
        .unwrap()
        .iter()
        .filter_map(|v| v.human_id().map(|h| h.as_str().to_owned()))
        .collect()
}
