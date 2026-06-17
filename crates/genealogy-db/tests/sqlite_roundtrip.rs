//! End-to-end SQLite round-trip for the Person aggregate (plan step 5).
//!
//! Initializes a fresh temp-file SQLite workspace, wires the `PersonView` projection through the
//! `WorkspaceStore`, executes commands, and asserts both the read model and the raw stored event
//! row. Proves: events persist and replay, the projection updates, a rejected command persists
//! nothing, and events are stored as internally-tagged JSON with their provenance envelope
//! (ADR 0004 §1, §4).

#![cfg(feature = "sqlite")]

use std::sync::Arc;

use cqrs_es::persist::{GenericQuery, ViewRepository};
use genealogy_core::enums::EvidenceLevel;
use genealogy_core::ids::{AgentId, AssertionId, HumanId, PersonId};
use genealogy_core::name::{NameType, PersonName, Surname};
use genealogy_core::person::command::{PersonCommand, PersonCommandEnvelope};
use genealogy_core::person::{PersonState, PersonView};
use genealogy_core::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
use genealogy_db::schema::{create_sqlite_view_table, init_sqlite};
use genealogy_db::{open_sqlite_pool, sqlite_store};
use sqlite_es::SqliteViewRepository;
use sqlx::{Pool, Row, Sqlite};
use time::macros::datetime;
use uuid::Uuid;

type PersonViewRepository = SqliteViewRepository<PersonView, PersonState>;

const VIEW_TABLE: &str = "person_view";

fn pid(n: u128) -> PersonId {
    PersonId::from_uuid(Uuid::from_u128(n))
}

fn meta(assertion: u128) -> AssertionMeta {
    AssertionMeta {
        assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
        context: EventContext {
            operator: Agent {
                kind: AgentKind::Human,
                id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                display: Some("Ada".to_owned()),
            },
            occurred_at: Timestamp::new(datetime!(2026-06-17 12:00:00 UTC)),
            rationale: Some("parish register".to_owned()),
            confidence: Confidence::High,
            citations: Vec::new(),
            evidence_analysis: None,
        },
    }
}

fn envelope(assertion: u128, command: PersonCommand) -> PersonCommandEnvelope {
    PersonCommandEnvelope {
        meta: meta(assertion),
        command,
    }
}

fn ada() -> PersonName {
    PersonName {
        name_type: NameType::BirthName,
        given: Some("Ada".to_owned()),
        surnames: vec![Surname {
            prefix: None,
            surname: "Lovelace".to_owned(),
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
    }
}

/// Builds a fresh workspace in a temp dir, returning the store, the view repo, and the pool.
#[expect(clippy::unwrap_used, reason = "test setup; a failure here should abort the test")]
async fn fresh_workspace() -> (
    genealogy_db::WorkspaceStore<PersonState>,
    Arc<PersonViewRepository>,
    Pool<Sqlite>,
    tempfile::TempDir,
) {
    let dir = tempfile::tempdir().unwrap();
    let db_path = dir.path().join("workspace.db");
    let connection = format!("sqlite://{}", db_path.display());

    let pool = open_sqlite_pool(&connection).await;
    init_sqlite(&pool).await.unwrap();
    create_sqlite_view_table(&pool, VIEW_TABLE).await.unwrap();

    let repo = Arc::new(PersonViewRepository::new(VIEW_TABLE, pool.clone()));
    let query = GenericQuery::new(repo.clone());
    let store = sqlite_store(pool.clone(), vec![Box::new(query)], ());
    (store, repo, pool, dir)
}

#[tokio::test]
async fn person_events_persist_and_drive_the_projection() {
    let (store, repo, _pool, _dir) = fresh_workspace().await;
    let id = pid(100).to_string();

    store
        .execute(
            &id,
            envelope(
                1,
                PersonCommand::CreatePerson {
                    person_id: pid(100),
                    human_id: HumanId::new("I0100"),
                    evidence_level: EvidenceLevel::Conclusion,
                },
            ),
        )
        .await
        .unwrap();

    store
        .execute(
            &id,
            envelope(
                2,
                PersonCommand::AssertName {
                    person_id: pid(100),
                    name: ada(),
                },
            ),
        )
        .await
        .unwrap();

    let view = repo.load(&id).await.unwrap().expect("view should exist");
    assert!(view.exists());
    assert_eq!(view.human_id().map(HumanId::as_str), Some("I0100"));
    assert_eq!(view.evidence_level(), Some(EvidenceLevel::Conclusion));
    let names = view.names();
    assert_eq!(names.len(), 1);
    assert_eq!(names[0].given.as_deref(), Some("Ada"));
}

#[tokio::test]
async fn a_rejected_command_persists_no_event() {
    let (store, repo, pool, _dir) = fresh_workspace().await;
    let id = pid(100).to_string();

    store
        .execute(
            &id,
            envelope(
                1,
                PersonCommand::CreatePerson {
                    person_id: pid(100),
                    human_id: HumanId::new("I0100"),
                    evidence_level: EvidenceLevel::Conclusion,
                },
            ),
        )
        .await
        .unwrap();

    // Re-creating the same person is a domain rejection.
    let err = store
        .execute(
            &id,
            envelope(
                2,
                PersonCommand::CreatePerson {
                    person_id: pid(100),
                    human_id: HumanId::new("I0100"),
                    evidence_level: EvidenceLevel::Conclusion,
                },
            ),
        )
        .await;
    assert!(err.is_err(), "expected the duplicate create to be rejected");

    // Exactly one event was committed (the original create); the rejection added nothing.
    let count: i64 = sqlx::query("SELECT COUNT(*) AS n FROM events WHERE aggregate_id = ?")
        .bind(&id)
        .fetch_one(&pool)
        .await
        .unwrap()
        .get("n");
    assert_eq!(count, 1);

    let view = repo.load(&id).await.unwrap().expect("view should exist");
    assert!(view.exists());
}

#[tokio::test]
async fn stored_events_are_internally_tagged_json_with_provenance() {
    let (store, _repo, pool, _dir) = fresh_workspace().await;
    let id = pid(100).to_string();

    store
        .execute(
            &id,
            envelope(
                1,
                PersonCommand::CreatePerson {
                    person_id: pid(100),
                    human_id: HumanId::new("I0100"),
                    evidence_level: EvidenceLevel::Conclusion,
                },
            ),
        )
        .await
        .unwrap();

    let row = sqlx::query("SELECT event_type, event_version, payload FROM events WHERE sequence = 1")
        .fetch_one(&pool)
        .await
        .unwrap();
    let event_type: String = row.get("event_type");
    let event_version: String = row.get("event_version");
    let payload: String = row.get("payload");
    assert_eq!(event_type, "PersonCreated");
    assert_eq!(event_version, "1.0");

    let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
    // Internally-tagged discriminator at the top level (ADR 0004 §4) ...
    assert_eq!(payload["type"], "PersonCreated");
    // ... and the provenance envelope travels in the payload (ADR 0004 §1).
    assert_eq!(payload["assertion_id"], Uuid::from_u128(1).to_string());
    assert_eq!(payload["context"]["confidence"], "High");
    assert_eq!(payload["context"]["operator"]["kind"]["kind"], "Human");
}
