//! Integration tests for the engine-neutral [`Store`] API. These use only the public surface —
//! no `sqlx`, `sqlite-es`, or `cqrs-es` types — proving the abstraction holds end to end.

#![cfg(feature = "sqlite")]
#![expect(clippy::unwrap_used, reason = "tests abort on setup/assertion failure")]

use genealogy_core::enums::EvidenceLevel;
use genealogy_core::id_format::IdFormat;
use genealogy_core::ids::{AgentId, AssertionId, HumanId, PersonId};
use genealogy_core::name::{NameType, PersonName, Surname};
use genealogy_core::person::command::{PersonCommand, PersonCommandEnvelope};
use genealogy_core::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
use genealogy_db::{CommandError, DbError, Store};
use time::macros::datetime;
use uuid::Uuid;

async fn store() -> (Store, tempfile::TempDir) {
    let dir = tempfile::tempdir().unwrap();
    let url = format!("sqlite://{}", dir.path().join("ws.sqlite3").display());
    (Store::open(&url).await.unwrap(), dir)
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
            confidence: Some(Confidence::Normal),
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

#[tokio::test]
async fn allocates_sequential_ids_and_survives_width_growth() {
    let (store, _dir) = store().await;
    assert_eq!(store.next_person_human_id(&person_format()).await.unwrap(), "I0001");

    create(&store, 1, "I0001").await;
    create(&store, 2, "I0002").await;
    assert_eq!(store.next_person_human_id(&person_format()).await.unwrap(), "I0003");

    // Past the zero-pad width, numbering must keep counting numerically, not lexicographically.
    create(&store, 9999, "I9999").await;
    assert_eq!(store.next_person_human_id(&person_format()).await.unwrap(), "I10000");
}

#[tokio::test]
async fn allocates_with_a_suffix_format() {
    let (store, _dir) = store().await;
    let format = IdFormat::parse("P-%03d").unwrap();
    assert_eq!(store.next_person_human_id(&format).await.unwrap(), "P-001");
    create(&store, 1, "P-001").await;
    assert_eq!(store.next_person_human_id(&format).await.unwrap(), "P-002");
}

#[tokio::test]
async fn find_and_list_reflect_the_projection() {
    let (store, _dir) = store().await;
    create(&store, 2, "I0002").await;
    name(&store, 2, "Alan", "Turing").await;
    create(&store, 1, "I0001").await;
    name(&store, 1, "Ada", "Lovelace").await;

    let found = store.find_person("I0001").await.unwrap().expect("exists");
    assert_eq!(found.human_id().map(HumanId::as_str), Some("I0001"));
    assert_eq!(found.names()[0].given.as_deref(), Some("Ada"));

    assert!(store.find_person("I0404").await.unwrap().is_none());

    let all = store.list_persons().await.unwrap();
    let ids: Vec<&str> = all.iter().filter_map(|v| v.human_id().map(HumanId::as_str)).collect();
    assert_eq!(ids, ["I0001", "I0002"]);
}

#[tokio::test]
async fn a_domain_rejection_is_distinct_from_an_infrastructure_error() {
    let (store, _dir) = store().await;
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

// Only meaningful when the postgres backend is NOT compiled in; with `--features postgres` a
// `postgres://` url opens a real connection (covered by `tests/postgres_store.rs`).
#[cfg(not(feature = "postgres"))]
#[tokio::test]
async fn postgres_url_is_reported_unsupported() {
    let err = Store::open("postgres://localhost/x").await;
    assert!(matches!(err, Err(DbError::Unsupported(_))));
}

#[tokio::test]
async fn an_unknown_scheme_is_malformed() {
    let err = Store::open("mysql://localhost/x").await;
    assert!(matches!(err, Err(DbError::Malformed(_))));
}
