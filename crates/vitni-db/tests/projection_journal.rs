//! A commit's events land before its projection is written (#536, ADR 0050): a process killed in
//! between leaves the view, and the indexes read from it, without the newest events. These tests put
//! a workspace in that state with SQL and check the next open makes it whole, on SQLite and (under
//! `--features postgres`, with a Docker daemon) Postgres.

#![cfg(any(feature = "sqlite", feature = "postgres"))]
#![expect(clippy::unwrap_used, reason = "tests abort on setup/assertion failure")]

use time::macros::datetime;
use uuid::Uuid;
use vitni_core::enums::{EventType, EvidenceLevel, ParticipantRole};
use vitni_core::event::command::{EventCommand, EventCommandEnvelope};
use vitni_core::ids::{AgentId, AssertionId, EventId, HumanId, PersonId};
use vitni_core::person::PersonError;
use vitni_core::person::command::{PersonCommand, PersonCommandEnvelope};
use vitni_core::provenance::{Agent, AgentKind, AssertionMeta, EventContext, Timestamp};
use vitni_db::{CommandError, ProjectionRow, RecordLink, Store};

/// The person every test changes.
const PERSON: u128 = 1;

fn meta(assertion: u128) -> AssertionMeta {
    AssertionMeta {
        assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
        context: EventContext {
            operator: Agent {
                kind: AgentKind::Human,
                id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                display: None,
            },
            occurred_at: Timestamp::new(datetime!(2026-10-12 12:00:00 UTC)),
            rationale: None,
            confidence: None,
            citations: Vec::new(),
            evidence_analysis: None,
            origin: None,
        },
    }
}

fn id(n: u128) -> String {
    Uuid::from_u128(n).to_string()
}

async fn create_event(store: &Store, n: u128) {
    let envelope = EventCommandEnvelope {
        meta: meta(n * 100),
        command: EventCommand::CreateEvent {
            event_id: EventId::from_uuid(Uuid::from_u128(n)),
            human_id: HumanId::new(format!("E{n:04}")),
            event_type: EventType::Birth,
        },
    };
    store.execute_event(&id(n), envelope).await.unwrap();
}

async fn person(store: &Store, assertion: u128, command: PersonCommand) -> Result<(), CommandError<PersonError>> {
    let envelope = PersonCommandEnvelope {
        meta: meta(assertion),
        command,
    };
    store.execute_person(&id(PERSON), envelope).await
}

/// The person takes part in `event`: an event on the person that changes its view and its links.
async fn take_part(store: &Store, event: u128) -> Result<(), CommandError<PersonError>> {
    let command = PersonCommand::AssertParticipation {
        person_id: PersonId::from_uuid(Uuid::from_u128(PERSON)),
        event_id: EventId::from_uuid(Uuid::from_u128(event)),
        role: ParticipantRole::Primary,
        age: None,
        attributes: Vec::new(),
        notes: Vec::new(),
    };
    person(store, event, command).await
}

/// Creates the person and three events, the person taking part in the first.
async fn seed(store: &Store) {
    for event in [20, 21, 22] {
        create_event(store, event).await;
    }
    let create = PersonCommand::CreatePerson {
        person_id: PersonId::from_uuid(Uuid::from_u128(PERSON)),
        human_id: HumanId::new("I0001"),
        evidence_level: EvidenceLevel::Conclusion,
        external_ids: Vec::new(),
    };
    person(store, 10, create).await.unwrap();
    take_part(store, 20).await.unwrap();
}

/// Copies the person's view row aside, so [`KILL_BEFORE_PROJECTION`] can put it back.
const KEEP_VIEW: &str = "CREATE TABLE kept_view AS SELECT view_id, sequence, payload FROM person_view";

/// Writes the kept row back over the person's view: the view as it was before the last commit.
const RESTORE_VIEW: [&str; 2] = [
    "UPDATE person_view SET sequence = kept_view.sequence, payload = kept_view.payload \
     FROM kept_view WHERE person_view.view_id = kept_view.view_id",
    "DROP TABLE kept_view",
];

/// Journals the person, as a command does before its events commit.
const JOURNAL_PERSON: &str = "INSERT INTO record_origins_pending (aggregate_type, aggregate_id) \
     VALUES ('person', '00000000-0000-0000-0000-000000000001')";

/// Drops the person's link to the event of the last commit, written from its view.
const DROP_LAST_LINK: &str = "DELETE FROM record_links WHERE target = '00000000-0000-0000-0000-000000000015'";

/// What a commit killed after its events and before its projection leaves: the view and its links
/// as they were, and the journal entry naming the person.
fn kill_before_projection() -> Vec<&'static str> {
    vec![RESTORE_VIEW[0], RESTORE_VIEW[1], DROP_LAST_LINK, JOURNAL_PERSON]
}

/// What the projections and the person's links read once whole.
#[derive(Debug, PartialEq)]
struct Derived {
    rows: Vec<ProjectionRow>,
    links: Vec<(String, String)>,
}

async fn derived(store: &Store) -> Derived {
    let mut links = store
        .linking(RecordLink::Participation, &[id(20), id(21), id(22)])
        .await
        .unwrap();
    links.sort();
    Derived {
        rows: store.projection_rows().await.unwrap(),
        links,
    }
}

/// The sequence of the last event the person's view holds.
async fn person_sequence(store: &Store) -> i64 {
    let rows = store.projection_rows().await.unwrap();
    rows.iter().find(|row| row.table == "person_view").unwrap().sequence
}

/// The tests, run once per engine over that engine's `Workspace`, made by `$new`: `open` opens a
/// [`Store`] on it and `run` executes SQL statements against it with the store closed.
macro_rules! journal_tests {
    ($new:expr) => {
        /// Seeds the workspace, keeps the person's view, and commits the person's part in event 21;
        /// returns what that leaves derived.
        async fn seed_and_take_part(workspace: &Workspace) -> super::Derived {
            let store = workspace.open().await;
            super::seed(&store).await;
            workspace.run(&[super::KEEP_VIEW]).await;
            super::take_part(&store, 21).await.unwrap();
            let derived = super::derived(&store).await;
            assert_eq!(
                super::person_sequence(&store).await,
                3,
                "created, two participations"
            );
            derived
        }

        #[tokio::test]
        async fn a_commit_killed_before_its_projection_write_is_whole_on_the_next_open() {
            let workspace = $new;
            let whole = seed_and_take_part(&workspace).await;
            workspace.run(&super::kill_before_projection()).await;
            assert_eq!(super::derived(&workspace.open().await).await, whole);
        }

        #[tokio::test]
        async fn a_journalled_commit_whose_projection_was_written_is_not_applied_twice() {
            let workspace = $new;
            let whole = seed_and_take_part(&workspace).await;
            workspace
                .run(&["DROP TABLE kept_view", super::JOURNAL_PERSON])
                .await;
            assert_eq!(super::derived(&workspace.open().await).await, whole);
        }

        #[tokio::test]
        async fn a_view_missing_earlier_events_fails_the_next_commit_and_the_next_open_repairs_it() {
            let uninterrupted = $new;
            seed_and_take_part(&uninterrupted).await;
            let store = uninterrupted.open().await;
            super::take_part(&store, 22).await.unwrap();
            let whole = super::derived(&store).await;

            let workspace = $new;
            seed_and_take_part(&workspace).await;
            workspace
                .run(&[
                    super::RESTORE_VIEW[0],
                    super::RESTORE_VIEW[1],
                    super::DROP_LAST_LINK,
                ])
                .await;
            let store = workspace.open().await;
            assert_eq!(
                super::person_sequence(&store).await,
                2,
                "nothing journalled the lost write"
            );
            let failed = super::take_part(&store, 22).await;
            assert!(
                matches!(failed, Err(vitni_db::CommandError::Store(_))),
                "{failed:?}"
            );
            drop(store);
            assert_eq!(super::derived(&workspace.open().await).await, whole);
        }

        #[tokio::test]
        async fn a_view_table_without_event_sequences_is_rebuilt() {
            let workspace = $new;
            let whole = seed_and_take_part(&workspace).await;
            workspace
                .run(&[
                    "DROP TABLE kept_view",
                    "ALTER TABLE person_view RENAME COLUMN sequence TO version",
                ])
                .await;
            assert_eq!(super::derived(&workspace.open().await).await, whole);
        }
    };
}

#[cfg(feature = "sqlite")]
mod sqlite {
    use vitni_db::Store;

    struct Workspace {
        _dir: tempfile::TempDir,
        url: String,
    }

    impl Workspace {
        fn new() -> Self {
            let dir = tempfile::tempdir().unwrap();
            let url = format!("sqlite://{}", dir.path().join("ws.sqlite3").display());
            Self { _dir: dir, url }
        }

        async fn open(&self) -> Store {
            Store::open(&self.url).await.unwrap()
        }

        async fn run(&self, statements: &[&str]) {
            let pool = sqlx::SqlitePool::connect(&self.url).await.unwrap();
            for statement in statements {
                sqlx::query(statement).execute(&pool).await.unwrap();
            }
            pool.close().await;
        }
    }

    journal_tests!(Workspace::new());
}

#[cfg(feature = "postgres")]
mod postgres {
    use sqlx::migrate::Migrator;
    use test_containers_util::sqlx_pg::PostgresTestDb;
    use vitni_db::Store;

    static MIGRATIONS: Migrator = sqlx::migrate!();

    struct Workspace {
        db: PostgresTestDb,
    }

    impl Workspace {
        async fn new() -> Self {
            Self {
                db: PostgresTestDb::create("vitni-pg", &MIGRATIONS, None, None).await,
            }
        }

        async fn open(&self) -> Store {
            Store::open(self.db.dsn()).await.unwrap()
        }

        async fn run(&self, statements: &[&str]) {
            let pool = sqlx::PgPool::connect(self.db.dsn()).await.unwrap();
            for statement in statements {
                sqlx::query(statement).execute(&pool).await.unwrap();
            }
            pool.close().await;
        }
    }

    journal_tests!(Workspace::new().await);
}
