//! SQLite backend for [`Store`](crate::store::Store) — private wiring (ADR 0002).
//!
//! Holds the `cqrs-es` framework for the Person aggregate and the connection pool its `person_view`
//! projection is queried through. Everything here (`sqlx`, `sqlite-es`, `cqrs-es`) is an
//! implementation detail; only [`crate::store`] re-exposes it, in engine-neutral terms.

use std::sync::Arc;

use cqrs_es::persist::{GenericQuery, PersistedEventStore};
use cqrs_es::{AggregateError, CqrsFramework};
use genealogy_core::id_format::IdFormat;
use genealogy_core::person::{PersonCommandEnvelope, PersonError, PersonState, PersonView};
use sqlite_es::{SqliteEventRepository, SqliteViewRepository, default_sqlite_pool, sqlite_cqrs};
use sqlx::{Pool, Sqlite};

use crate::query;
use crate::schema;
use crate::store::{CommandError, DbError};

/// The Person conclusion projection table written by the `GenericQuery`.
pub(crate) const PERSON_VIEW_TABLE: &str = "person_view";

type PersonCqrs = CqrsFramework<PersonState, PersistedEventStore<SqliteEventRepository, PersonState>>;
type PersonViewRepository = SqliteViewRepository<PersonView, PersonState>;

/// A SQLite-backed Person store: the command framework plus the read-model pool.
pub(crate) struct SqliteStore {
    cqrs: PersonCqrs,
    pool: Pool<Sqlite>,
}

impl SqliteStore {
    /// Opens the pool for `database_url`, runs the (idempotent) DDL, and wires the projection.
    pub(crate) async fn open(database_url: &str) -> Result<Self, DbError> {
        let pool = default_sqlite_pool(database_url).await;
        schema::init_sqlite(&pool)
            .await
            .map_err(|e| DbError::Backend(format!("initializing event store: {e}")))?;
        schema::create_sqlite_view_table(&pool, PERSON_VIEW_TABLE)
            .await
            .map_err(|e| DbError::Backend(format!("creating projection table: {e}")))?;

        let repo = Arc::new(PersonViewRepository::new(PERSON_VIEW_TABLE, pool.clone()));
        let cqrs = sqlite_cqrs(pool.clone(), vec![Box::new(GenericQuery::new(repo))], ());
        Ok(Self { cqrs, pool })
    }

    pub(crate) async fn execute_person(
        &self,
        aggregate_id: &str,
        command: PersonCommandEnvelope,
    ) -> Result<(), CommandError> {
        self.cqrs
            .execute(aggregate_id, command)
            .await
            .map_err(map_aggregate_error)
    }

    pub(crate) async fn next_person_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        query::next_person_human_id(&self.pool, format).await
    }

    pub(crate) async fn find_person(&self, human_id: &str) -> Result<Option<PersonView>, DbError> {
        query::find_person_by_human_id(&self.pool, human_id).await
    }

    pub(crate) async fn list_persons(&self) -> Result<Vec<PersonView>, DbError> {
        query::list_person_views(&self.pool).await
    }
}

/// Maps a `cqrs-es` framework error to the neutral [`CommandError`], keeping rejection distinct.
fn map_aggregate_error(error: AggregateError<PersonError>) -> CommandError {
    match error {
        AggregateError::UserError(domain) => CommandError::Rejected(domain),
        AggregateError::AggregateConflict => {
            CommandError::Store(DbError::Backend("aggregate version conflict".to_owned()))
        }
        AggregateError::DatabaseConnectionError(source)
        | AggregateError::DeserializationError(source)
        | AggregateError::UnexpectedError(source) => CommandError::Store(DbError::Backend(source.to_string())),
    }
}

#[cfg(test)]
mod tests {
    use super::SqliteStore;
    use genealogy_core::enums::EvidenceLevel;
    use genealogy_core::ids::{AgentId, AssertionId, HumanId, PersonId};
    use genealogy_core::person::command::{PersonCommand, PersonCommandEnvelope};
    use genealogy_core::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use sqlx::Row;
    use time::macros::datetime;
    use uuid::Uuid;

    async fn store() -> (SqliteStore, tempfile::TempDir) {
        let dir = tempfile::tempdir().unwrap();
        let url = format!("sqlite://{}", dir.path().join("ws.sqlite3").display());
        let store = SqliteStore::open(&url).await.unwrap();
        (store, dir)
    }

    #[tokio::test]
    async fn a_stored_event_carries_its_provenance_in_the_payload() {
        let (store, _dir) = store().await;
        let person_id = PersonId::from_uuid(Uuid::from_u128(1));
        let envelope = PersonCommandEnvelope {
            meta: AssertionMeta {
                assertion_id: AssertionId::from_uuid(Uuid::from_u128(2)),
                context: EventContext {
                    operator: Agent {
                        kind: AgentKind::Human,
                        id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                        display: Some("Ada".to_owned()),
                    },
                    occurred_at: Timestamp::new(datetime!(2026-06-18 12:00:00 UTC)),
                    rationale: None,
                    confidence: Confidence::High,
                    citations: Vec::new(),
                    evidence_analysis: None,
                },
            },
            command: PersonCommand::CreatePerson {
                person_id,
                human_id: HumanId::new("I0001"),
                evidence_level: EvidenceLevel::Conclusion,
            },
        };
        store.execute_person(&person_id.to_string(), envelope).await.unwrap();

        // The provenance envelope travels in the event payload (ADR 0004 §1, §4), as flat
        // internally-tagged JSON.
        let payload: String = sqlx::query("SELECT payload FROM events WHERE sequence = 1")
            .fetch_one(&store.pool)
            .await
            .unwrap()
            .get("payload");
        let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(payload["type"], "PersonCreated");
        assert_eq!(payload["assertion_id"], Uuid::from_u128(2).to_string());
        assert_eq!(payload["context"]["confidence"], "High");
        assert_eq!(payload["context"]["operator"]["kind"]["kind"], "Human");
    }
}
