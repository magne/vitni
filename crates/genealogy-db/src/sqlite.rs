//! SQLite backend for [`Store`](crate::store::Store) — private wiring (ADR 0002).
//!
//! Holds the `cqrs-es` framework for the Person aggregate and the connection pool its `person_view`
//! projection is queried through. Everything here (`sqlx`, `sqlite-es`, `cqrs-es`) is an
//! implementation detail; only [`crate::store`] re-exposes it, in engine-neutral terms.

use std::sync::Arc;

use cqrs_es::persist::{GenericQuery, PersistedEventStore};
use cqrs_es::{AggregateError, CqrsFramework};
use genealogy_core::citation::{CitationCommandEnvelope, CitationError, CitationState, CitationView};
use genealogy_core::event::{EventCommandEnvelope, EventError, EventState, EventView};
use genealogy_core::family::{FamilyCommandEnvelope, FamilyError, FamilyState, FamilyView};
use genealogy_core::id_format::IdFormat;
use genealogy_core::person::{PersonCommandEnvelope, PersonError, PersonState, PersonView};
use genealogy_core::place::{PlaceCommandEnvelope, PlaceError, PlaceState, PlaceView};
use genealogy_core::source::{SourceCommandEnvelope, SourceError, SourceState, SourceView};
use sqlite_es::{SqliteEventRepository, SqliteViewRepository, default_sqlite_pool, sqlite_cqrs};
use sqlx::{Pool, Sqlite};

use crate::query;
use crate::resolver::{SqliteCitationRefResolver, SqliteEventRefResolver};
use crate::schema;
use crate::store::{CommandError, DbError};

/// The Person conclusion projection table written by the `GenericQuery`.
pub(crate) const PERSON_VIEW_TABLE: &str = "person_view";
/// The Family conclusion projection table written by the `GenericQuery`.
pub(crate) const FAMILY_VIEW_TABLE: &str = "family_view";
/// The Place conclusion projection table written by the `GenericQuery`.
pub(crate) const PLACE_VIEW_TABLE: &str = "place_view";
/// The Source conclusion projection table written by the `GenericQuery`.
pub(crate) const SOURCE_VIEW_TABLE: &str = "source_view";
/// The Citation conclusion projection table written by the `GenericQuery`.
pub(crate) const CITATION_VIEW_TABLE: &str = "citation_view";
/// The Event conclusion projection table written by the `GenericQuery`.
pub(crate) const EVENT_VIEW_TABLE: &str = "event_view";

type PersonCqrs = CqrsFramework<PersonState, PersistedEventStore<SqliteEventRepository, PersonState>>;
type PersonViewRepository = SqliteViewRepository<PersonView, PersonState>;
type FamilyCqrs = CqrsFramework<FamilyState, PersistedEventStore<SqliteEventRepository, FamilyState>>;
type FamilyViewRepository = SqliteViewRepository<FamilyView, FamilyState>;
type PlaceCqrs = CqrsFramework<PlaceState, PersistedEventStore<SqliteEventRepository, PlaceState>>;
type PlaceViewRepository = SqliteViewRepository<PlaceView, PlaceState>;
type SourceCqrs = CqrsFramework<SourceState, PersistedEventStore<SqliteEventRepository, SourceState>>;
type SourceViewRepository = SqliteViewRepository<SourceView, SourceState>;
type CitationCqrs = CqrsFramework<CitationState, PersistedEventStore<SqliteEventRepository, CitationState>>;
type CitationViewRepository = SqliteViewRepository<CitationView, CitationState>;
type EventCqrs = CqrsFramework<EventState, PersistedEventStore<SqliteEventRepository, EventState>>;
type EventViewRepository = SqliteViewRepository<EventView, EventState>;

/// A SQLite-backed store: one command framework per aggregate, sharing the read-model pool.
pub(crate) struct SqliteStore {
    person_cqrs: PersonCqrs,
    family_cqrs: FamilyCqrs,
    place_cqrs: PlaceCqrs,
    source_cqrs: SourceCqrs,
    citation_cqrs: CitationCqrs,
    event_cqrs: EventCqrs,
    pool: Pool<Sqlite>,
}

impl SqliteStore {
    /// Opens the pool for `database_url`, runs the (idempotent) DDL, and wires the projections.
    pub(crate) async fn open(database_url: &str) -> Result<Self, DbError> {
        let pool = default_sqlite_pool(database_url).await;
        schema::init_sqlite(&pool)
            .await
            .map_err(|e| DbError::Backend(format!("initializing event store: {e}")))?;
        for table in [
            PERSON_VIEW_TABLE,
            FAMILY_VIEW_TABLE,
            PLACE_VIEW_TABLE,
            SOURCE_VIEW_TABLE,
            CITATION_VIEW_TABLE,
            EVENT_VIEW_TABLE,
        ] {
            schema::create_sqlite_view_table(&pool, table)
                .await
                .map_err(|e| DbError::Backend(format!("creating projection table {table}: {e}")))?;
        }

        let person_repo = Arc::new(PersonViewRepository::new(PERSON_VIEW_TABLE, pool.clone()));
        let person_cqrs = sqlite_cqrs(pool.clone(), vec![Box::new(GenericQuery::new(person_repo))], ());
        let family_repo = Arc::new(FamilyViewRepository::new(FAMILY_VIEW_TABLE, pool.clone()));
        let family_cqrs = sqlite_cqrs(pool.clone(), vec![Box::new(GenericQuery::new(family_repo))], ());
        let place_repo = Arc::new(PlaceViewRepository::new(PLACE_VIEW_TABLE, pool.clone()));
        let place_cqrs = sqlite_cqrs(pool.clone(), vec![Box::new(GenericQuery::new(place_repo))], ());
        let source_repo = Arc::new(SourceViewRepository::new(SOURCE_VIEW_TABLE, pool.clone()));
        let source_cqrs = sqlite_cqrs(pool.clone(), vec![Box::new(GenericQuery::new(source_repo))], ());
        // The Citation aggregate's `Services` is a resolver that reads the Source projection to
        // answer the `UnknownSource` aggregate-tax check (ADR 0004 §3).
        let citation_repo = Arc::new(CitationViewRepository::new(CITATION_VIEW_TABLE, pool.clone()));
        let citation_cqrs = sqlite_cqrs(
            pool.clone(),
            vec![Box::new(GenericQuery::new(citation_repo))],
            SqliteCitationRefResolver::new(pool.clone()),
        );
        // The Event aggregate's `Services` resolver reads the Place projection for the
        // `UnknownPlace` aggregate-tax check (ADR 0004 §3).
        let event_repo = Arc::new(EventViewRepository::new(EVENT_VIEW_TABLE, pool.clone()));
        let event_cqrs = sqlite_cqrs(
            pool.clone(),
            vec![Box::new(GenericQuery::new(event_repo))],
            SqliteEventRefResolver::new(pool.clone()),
        );
        Ok(Self {
            person_cqrs,
            family_cqrs,
            place_cqrs,
            source_cqrs,
            citation_cqrs,
            event_cqrs,
            pool,
        })
    }

    pub(crate) async fn execute_person(
        &self,
        aggregate_id: &str,
        command: PersonCommandEnvelope,
    ) -> Result<(), CommandError<PersonError>> {
        self.person_cqrs
            .execute(aggregate_id, command)
            .await
            .map_err(map_aggregate_error)
    }

    pub(crate) async fn next_person_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        query::next_human_id(&self.pool, PERSON_VIEW_TABLE, format).await
    }

    pub(crate) async fn find_person(&self, human_id: &str) -> Result<Option<PersonView>, DbError> {
        query::find_view_by_human_id(&self.pool, PERSON_VIEW_TABLE, human_id).await
    }

    pub(crate) async fn list_persons(&self) -> Result<Vec<PersonView>, DbError> {
        query::list_views(&self.pool, PERSON_VIEW_TABLE).await
    }

    pub(crate) async fn execute_family(
        &self,
        aggregate_id: &str,
        command: FamilyCommandEnvelope,
    ) -> Result<(), CommandError<FamilyError>> {
        self.family_cqrs
            .execute(aggregate_id, command)
            .await
            .map_err(map_aggregate_error)
    }

    pub(crate) async fn next_family_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        query::next_human_id(&self.pool, FAMILY_VIEW_TABLE, format).await
    }

    pub(crate) async fn find_family(&self, human_id: &str) -> Result<Option<FamilyView>, DbError> {
        query::find_view_by_human_id(&self.pool, FAMILY_VIEW_TABLE, human_id).await
    }

    pub(crate) async fn list_families(&self) -> Result<Vec<FamilyView>, DbError> {
        query::list_views(&self.pool, FAMILY_VIEW_TABLE).await
    }

    pub(crate) async fn execute_place(
        &self,
        aggregate_id: &str,
        command: PlaceCommandEnvelope,
    ) -> Result<(), CommandError<PlaceError>> {
        self.place_cqrs
            .execute(aggregate_id, command)
            .await
            .map_err(map_aggregate_error)
    }

    pub(crate) async fn next_place_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        query::next_human_id(&self.pool, PLACE_VIEW_TABLE, format).await
    }

    pub(crate) async fn find_place(&self, human_id: &str) -> Result<Option<PlaceView>, DbError> {
        query::find_view_by_human_id(&self.pool, PLACE_VIEW_TABLE, human_id).await
    }

    pub(crate) async fn list_places(&self) -> Result<Vec<PlaceView>, DbError> {
        query::list_views(&self.pool, PLACE_VIEW_TABLE).await
    }

    pub(crate) async fn execute_source(
        &self,
        aggregate_id: &str,
        command: SourceCommandEnvelope,
    ) -> Result<(), CommandError<SourceError>> {
        self.source_cqrs
            .execute(aggregate_id, command)
            .await
            .map_err(map_aggregate_error)
    }

    pub(crate) async fn next_source_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        query::next_human_id(&self.pool, SOURCE_VIEW_TABLE, format).await
    }

    pub(crate) async fn find_source(&self, human_id: &str) -> Result<Option<SourceView>, DbError> {
        query::find_view_by_human_id(&self.pool, SOURCE_VIEW_TABLE, human_id).await
    }

    pub(crate) async fn list_sources(&self) -> Result<Vec<SourceView>, DbError> {
        query::list_views(&self.pool, SOURCE_VIEW_TABLE).await
    }

    pub(crate) async fn execute_citation(
        &self,
        aggregate_id: &str,
        command: CitationCommandEnvelope,
    ) -> Result<(), CommandError<CitationError>> {
        self.citation_cqrs
            .execute(aggregate_id, command)
            .await
            .map_err(map_aggregate_error)
    }

    pub(crate) async fn next_citation_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        query::next_human_id(&self.pool, CITATION_VIEW_TABLE, format).await
    }

    pub(crate) async fn find_citation(&self, human_id: &str) -> Result<Option<CitationView>, DbError> {
        query::find_view_by_human_id(&self.pool, CITATION_VIEW_TABLE, human_id).await
    }

    pub(crate) async fn list_citations(&self) -> Result<Vec<CitationView>, DbError> {
        query::list_views(&self.pool, CITATION_VIEW_TABLE).await
    }

    pub(crate) async fn execute_event(
        &self,
        aggregate_id: &str,
        command: EventCommandEnvelope,
    ) -> Result<(), CommandError<EventError>> {
        self.event_cqrs
            .execute(aggregate_id, command)
            .await
            .map_err(map_aggregate_error)
    }

    pub(crate) async fn next_event_human_id(&self, format: &IdFormat) -> Result<String, DbError> {
        query::next_human_id(&self.pool, EVENT_VIEW_TABLE, format).await
    }

    pub(crate) async fn find_event(&self, human_id: &str) -> Result<Option<EventView>, DbError> {
        query::find_view_by_human_id(&self.pool, EVENT_VIEW_TABLE, human_id).await
    }

    pub(crate) async fn list_events(&self) -> Result<Vec<EventView>, DbError> {
        query::list_views(&self.pool, EVENT_VIEW_TABLE).await
    }
}

/// Maps a `cqrs-es` framework error to the neutral [`CommandError`], keeping rejection distinct.
/// Generic over the aggregate's domain error so every aggregate reuses one mapping.
fn map_aggregate_error<E: std::error::Error + 'static>(error: AggregateError<E>) -> CommandError<E> {
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
    use crate::store::CommandError;
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

    /// A minimal assertion meta for command tests (the application layer builds this — ADR 0004 §3).
    fn meta(assertion: u128) -> AssertionMeta {
        AssertionMeta {
            assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
            context: EventContext {
                operator: Agent {
                    kind: AgentKind::Human,
                    id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                    display: None,
                },
                occurred_at: Timestamp::new(datetime!(2026-06-19 12:00:00 UTC)),
                rationale: None,
                confidence: Confidence::Normal,
                citations: Vec::new(),
                evidence_analysis: None,
            },
        }
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

    #[tokio::test]
    async fn a_family_event_is_stored_under_the_family_aggregate_with_its_provenance() {
        use genealogy_core::family::command::{FamilyCommand, FamilyCommandEnvelope};
        use genealogy_core::ids::FamilyId;

        let (store, _dir) = store().await;
        let family_id = FamilyId::from_uuid(Uuid::from_u128(1));
        let envelope = FamilyCommandEnvelope {
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
            command: FamilyCommand::CreateFamily {
                family_id,
                human_id: HumanId::new("F0001"),
            },
        };
        store.execute_family(&family_id.to_string(), envelope).await.unwrap();

        // The event is keyed under the family aggregate, with provenance in the flat payload.
        let row = sqlx::query("SELECT aggregate_type, payload FROM events WHERE sequence = 1")
            .fetch_one(&store.pool)
            .await
            .unwrap();
        let aggregate_type: String = row.get("aggregate_type");
        assert_eq!(aggregate_type, "family");
        let payload: String = row.get("payload");
        let payload: serde_json::Value = serde_json::from_str(&payload).unwrap();
        assert_eq!(payload["type"], "FamilyCreated");
        assert_eq!(payload["assertion_id"], Uuid::from_u128(2).to_string());
        assert_eq!(payload["context"]["confidence"], "High");

        // And the projection is readable back through the neutral query path.
        let view = store.find_family("F0001").await.unwrap().expect("family projected");
        assert_eq!(view.human_id().map(ToString::to_string), Some("F0001".to_owned()));
    }

    #[tokio::test]
    async fn citation_against_a_missing_source_is_rejected_through_services() {
        use genealogy_core::citation::command::{CitationCommand, CitationCommandEnvelope};
        use genealogy_core::citation::error::CitationError;
        use genealogy_core::ids::{CitationId, SourceId};

        let (store, _dir) = store().await;
        // No source has been created, so the Services resolver reports it absent and the pure
        // `decide` rejects with the domain error — proving the aggregate-tax path (ADR 0004 §3),
        // not an app-layer guard.
        let citation_id = CitationId::from_uuid(Uuid::from_u128(1));
        let missing_source = SourceId::from_uuid(Uuid::from_u128(999));
        let envelope = CitationCommandEnvelope {
            meta: meta(2),
            command: CitationCommand::CreateCitation {
                citation_id,
                human_id: HumanId::new("C0001"),
                source_id: missing_source,
            },
        };
        let err = store
            .execute_citation(&citation_id.to_string(), envelope)
            .await
            .unwrap_err();
        assert!(
            matches!(err, CommandError::Rejected(CitationError::UnknownSource(s)) if s == missing_source),
            "expected UnknownSource, got {err:?}"
        );
    }

    #[tokio::test]
    async fn event_linking_a_missing_place_is_rejected_through_services() {
        use genealogy_core::enums::EventType;
        use genealogy_core::event::command::{EventCommand, EventCommandEnvelope};
        use genealogy_core::event::error::EventError;
        use genealogy_core::ids::{EventId, PlaceId};

        let (store, _dir) = store().await;
        let event_id = EventId::from_uuid(Uuid::from_u128(1));
        store
            .execute_event(
                &event_id.to_string(),
                EventCommandEnvelope {
                    meta: meta(2),
                    command: EventCommand::CreateEvent {
                        event_id,
                        human_id: HumanId::new("E0001"),
                        event_type: EventType::Birth,
                        private: false,
                    },
                },
            )
            .await
            .unwrap();

        // No place exists, so the resolver reports it absent and `decide` rejects — the aggregate
        // tax path (ADR 0004 §3), not an app guard.
        let missing_place = PlaceId::from_uuid(Uuid::from_u128(999));
        let err = store
            .execute_event(
                &event_id.to_string(),
                EventCommandEnvelope {
                    meta: meta(3),
                    command: EventCommand::LinkPlace {
                        event_id,
                        place_id: missing_place,
                    },
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, CommandError::Rejected(EventError::UnknownPlace(p)) if p == missing_place),
            "expected UnknownPlace, got {err:?}"
        );
    }

    #[tokio::test]
    async fn event_linking_a_present_place_succeeds_and_projects() {
        use genealogy_core::enums::{EventType, PlaceType};
        use genealogy_core::event::command::{EventCommand, EventCommandEnvelope};
        use genealogy_core::ids::{EventId, PlaceId};
        use genealogy_core::place::command::{PlaceCommand, PlaceCommandEnvelope};

        let (store, _dir) = store().await;
        let place_id = PlaceId::from_uuid(Uuid::from_u128(1));
        store
            .execute_place(
                &place_id.to_string(),
                PlaceCommandEnvelope {
                    meta: meta(2),
                    command: PlaceCommand::CreatePlace {
                        place_id,
                        human_id: HumanId::new("P0001"),
                        place_type: PlaceType::Parish,
                    },
                },
            )
            .await
            .unwrap();

        let event_id = EventId::from_uuid(Uuid::from_u128(2));
        for command in [
            EventCommand::CreateEvent {
                event_id,
                human_id: HumanId::new("E0001"),
                event_type: EventType::Birth,
                private: false,
            },
            EventCommand::LinkPlace { event_id, place_id },
        ] {
            store
                .execute_event(&event_id.to_string(), EventCommandEnvelope { meta: meta(3), command })
                .await
                .unwrap();
        }

        let view = store.find_event("E0001").await.unwrap().expect("event projected");
        assert_eq!(view.place_id(), Some(place_id));
    }

    #[tokio::test]
    async fn citation_against_a_present_source_succeeds_and_projects() {
        use genealogy_core::citation::command::{CitationCommand, CitationCommandEnvelope};
        use genealogy_core::ids::{CitationId, SourceId};
        use genealogy_core::source::command::{SourceCommand, SourceCommandEnvelope};

        let (store, _dir) = store().await;
        // Create the source first; its projection commits before the citation is executed, so the
        // resolver sees it.
        let source_id = SourceId::from_uuid(Uuid::from_u128(1));
        store
            .execute_source(
                &source_id.to_string(),
                SourceCommandEnvelope {
                    meta: meta(2),
                    command: SourceCommand::CreateSource {
                        source_id,
                        human_id: HumanId::new("S0001"),
                    },
                },
            )
            .await
            .unwrap();

        let citation_id = CitationId::from_uuid(Uuid::from_u128(2));
        store
            .execute_citation(
                &citation_id.to_string(),
                CitationCommandEnvelope {
                    meta: meta(3),
                    command: CitationCommand::CreateCitation {
                        citation_id,
                        human_id: HumanId::new("C0001"),
                        source_id,
                    },
                },
            )
            .await
            .unwrap();

        let view = store.find_citation("C0001").await.unwrap().expect("citation projected");
        assert_eq!(view.source_id(), Some(source_id));
    }
}
