//! SQLite backend for [`Store`](crate::store::Store) — private wiring (ADR 0002).
//!
//! Holds one `cqrs-es` framework per aggregate and the connection pool their projections are
//! queried through. The per-aggregate fields, wiring, methods, and rebuild are generated from the
//! [`registry`](crate::registry). Everything here (`sqlx`, `sqlite-es`, `cqrs-es`) is an
//! implementation detail; only [`crate::store`] re-exposes it, in engine-neutral terms.

use std::sync::Arc;

use cqrs_es::persist::{EventUpcaster, GenericQuery, PersistedEventStore, QueryReplay};
use cqrs_es::{Aggregate, CqrsFramework, View};
use sqlite_es::{SqliteEventRepository, SqliteViewRepository, default_sqlite_pool, sqlite_cqrs};
use sqlx::{Pool, Sqlite};

use crate::query;
use crate::registry::{for_each_db_aggregate, for_each_db_external_id_aggregate, for_each_db_human_id_aggregate};
use crate::resolver::SqliteRefStore;
use crate::schema;
use crate::store::{CommandError, DbError, map_aggregate_error};
use crate::tables::{
    ALL_VIEW_TABLES, CITATION_VIEW_TABLE, DNA_MATCH_VIEW_TABLE, DNA_TEST_VIEW_TABLE, EVENT_VIEW_TABLE,
    FAMILY_VIEW_TABLE, MEDIA_VIEW_TABLE, NOTE_VIEW_TABLE, PERSON_VIEW_TABLE, PLACE_VIEW_TABLE, REPOSITORY_VIEW_TABLE,
    SOURCE_VIEW_TABLE, TAG_VIEW_TABLE,
};

/// Builds one aggregate's `CqrsFramework` in `open()`, matching the registry `wiring` column: a
/// plain unit `Services`, a projection-reading resolver (the §9 aggregate tax), or the
/// hand-assembled Event store that carries upcasters at load (ADR 0010).
macro_rules! sqlite_open_cqrs {
    ($pool:ident, $repo:ident, (plain)) => {
        sqlite_cqrs($pool.clone(), vec![Box::new(GenericQuery::new($repo))], ())
    };
    ($pool:ident, $repo:ident, (resolver $resolver:path)) => {
        sqlite_cqrs(
            $pool.clone(),
            vec![Box::new(GenericQuery::new($repo))],
            <$resolver>::new(SqliteRefStore::shared($pool.clone())),
        )
    };
    ($pool:ident, $repo:ident, (event $resolver:path)) => {{
        let store = PersistedEventStore::new_event_store(SqliteEventRepository::new($pool.clone()))
            .with_upcasters(genealogy_core::event::upcasters());
        CqrsFramework::new(
            store,
            vec![Box::new(GenericQuery::new($repo))],
            <$resolver>::new(SqliteRefStore::shared($pool.clone())),
        )
    }};
}

/// Selects the read-model lookup for `find_*`, keyed by the registry `find_param` column: Tag is
/// keyed by its own id (`find_view_by_id`), every other aggregate by its `human_id`.
macro_rules! sqlite_find_query {
    ($pool:expr, $table:expr, human_id, $value:expr) => {
        query::find_view_by_human_id($pool, $table, $value)
    };
    ($pool:expr, $table:expr, tag_id, $value:expr) => {
        query::find_view_by_id($pool, $table, $value)
    };
}

/// Generates the SQLite backend from the registry: the per-aggregate `CqrsFramework` fields,
/// `open()` wiring, the command/find/list methods, and the rebuild loop. The projection-table
/// constants come from [`crate::tables`].
macro_rules! sqlite_store {
    ($(($snake:ident, $State:ty, $View:ty, $Cmd:ty, $Err:ty, $table_const:ident, $table_str:literal, $execute:ident, $find:ident, $find_param:ident, $list:ident, $wiring:tt, $upcasters:expr,)),+ $(,)?) => {
        /// A SQLite-backed store: one command framework per aggregate, sharing the read-model pool.
        pub(crate) struct SqliteStore {
            $(
                $snake: CqrsFramework<$State, PersistedEventStore<SqliteEventRepository, $State>>,
            )+
            pool: Pool<Sqlite>,
        }

        impl SqliteStore {
            /// Opens the pool for `database_url`, runs the (idempotent) DDL, and wires the projections.
            pub(crate) async fn open(database_url: &str) -> Result<Self, DbError> {
                let pool = default_sqlite_pool(database_url).await;
                schema::init_sqlite(&pool)
                    .await
                    .map_err(|e| DbError::Backend(format!("initializing event store: {e}")))?;
                for &table in ALL_VIEW_TABLES {
                    schema::create_sqlite_view_table(&pool, table)
                        .await
                        .map_err(|e| DbError::Backend(format!("creating projection table {table}: {e}")))?;
                }
                $(
                    let repo = Arc::new(SqliteViewRepository::<$View, $State>::new($table_const, pool.clone()));
                    let $snake = sqlite_open_cqrs!(pool, repo, $wiring);
                )+
                Ok(Self { $($snake,)+ pool })
            }

            $(
                pub(crate) async fn $execute(
                    &self,
                    aggregate_id: &str,
                    command: $Cmd,
                ) -> Result<(), CommandError<$Err>> {
                    self.$snake.execute(aggregate_id, command).await.map_err(map_aggregate_error)
                }

                pub(crate) async fn $find(&self, $find_param: &str) -> Result<Option<$View>, DbError> {
                    sqlite_find_query!(&self.pool, $table_const, $find_param, $find_param).await
                }

                pub(crate) async fn $list(&self) -> Result<Vec<$View>, DbError> {
                    query::list_views(&self.pool, $table_const).await
                }
            )+

            /// Rebuilds every projection from the event log (ADR 0010): each view table is cleared,
            /// then its aggregate's full history is replayed back into it through the same
            /// `GenericQuery` the live store uses, with the Event aggregate's upcasters applied. A
            /// maintenance operation — the caller must ensure no commands run concurrently.
            pub(crate) async fn rebuild_projections(&self) -> Result<(), DbError> {
                $(
                    rebuild_view::<$State, $View>(&self.pool, $table_const, $upcasters).await?;
                )+
                Ok(())
            }
        }
    };
}

for_each_db_aggregate!(sqlite_store);

/// Generates the per-aggregate `next_*_human_id` allocators (every aggregate but Tag).
macro_rules! sqlite_next_methods {
    ($(($snake:ident, $next:ident, $table_const:ident)),+ $(,)?) => {
        impl SqliteStore {
            $(
                pub(crate) async fn $next(&self, format: &genealogy_core::id_format::IdFormat) -> Result<String, DbError> {
                    query::next_human_id(&self.pool, $table_const, format).await
                }
            )+
        }
    };
}

for_each_db_human_id_aggregate!(sqlite_next_methods);

/// Generates the per-aggregate `find_*_by_external_id` lookups for the aggregates that carry
/// external ids (data-model §11).
macro_rules! sqlite_external_id_methods {
    ($(($snake:ident, $find:ident, $table_const:ident, $View:ty)),+ $(,)?) => {
        impl SqliteStore {
            $(
                pub(crate) async fn $find(&self, authority: &str, value: &str) -> Result<Option<$View>, DbError> {
                    query::find_view_by_external_id(&self.pool, $table_const, authority, value).await
                }
            )+
        }
    };
}

for_each_db_external_id_aggregate!(sqlite_external_id_methods);

/// Clears one view table and replays its aggregate's full event log back into it (ADR 0010).
///
/// `upcasters` migrate historical payloads during the replay; pass an empty vec for aggregates
/// whose schema has not evolved. `stream_all_events::<A>()` binds the aggregate type, so each
/// replay sees only its own events.
async fn rebuild_view<A, V>(
    pool: &Pool<Sqlite>,
    table: &str,
    upcasters: Vec<Box<dyn EventUpcaster>>,
) -> Result<(), DbError>
where
    A: Aggregate,
    V: View<A>,
{
    schema::clear_sqlite_view_table(pool, table)
        .await
        .map_err(|e| DbError::Backend(format!("clearing projection {table}: {e}")))?;
    let repo = Arc::new(SqliteViewRepository::<V, A>::new(table, pool.clone()));
    let replay =
        QueryReplay::new(SqliteEventRepository::new(pool.clone()), GenericQuery::new(repo)).with_upcasters(upcasters);
    replay
        .replay_all()
        .await
        .map_err(|e| DbError::Backend(format!("rebuilding projection {table}: {e}")))
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
    async fn a_person_is_found_by_its_external_id() {
        use genealogy_core::text::ExternalId;

        let (store, _dir) = store().await;
        let person_id = PersonId::from_uuid(Uuid::from_u128(1));
        for command in [
            PersonCommand::CreatePerson {
                person_id,
                human_id: HumanId::new("I0001"),
                evidence_level: EvidenceLevel::Persona,
            },
            PersonCommand::AddExternalId {
                person_id,
                external_id: ExternalId {
                    authority: "gedcom-uid".to_owned(),
                    value: "ABC-123".to_owned(),
                    kind: None,
                    url: None,
                },
            },
        ] {
            store
                .execute_person(&person_id.to_string(), PersonCommandEnvelope { meta: meta(2), command })
                .await
                .unwrap();
        }

        let found = store
            .find_person_by_external_id("gedcom-uid", "ABC-123")
            .await
            .unwrap()
            .expect("person resolved by external id");
        assert_eq!(found.human_id().map(ToString::to_string), Some("I0001".to_owned()));

        // A different authority/value does not match.
        assert!(
            store
                .find_person_by_external_id("gedcom-uid", "other")
                .await
                .unwrap()
                .is_none()
        );
        assert!(
            store
                .find_person_by_external_id("other", "ABC-123")
                .await
                .unwrap()
                .is_none()
        );
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
    async fn source_linking_a_missing_repository_is_rejected_through_services() {
        use genealogy_core::enums::SourceMediaType;
        use genealogy_core::ids::{RepositoryId, SourceId};
        use genealogy_core::repo_ref::RepoRef;
        use genealogy_core::source::command::{SourceCommand, SourceCommandEnvelope};
        use genealogy_core::source::error::SourceError;

        let (store, _dir) = store().await;
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

        // No repository exists, so the resolver reports it absent and `decide` rejects — the
        // aggregate tax path (ADR 0004 §3), not an app guard.
        let missing_repository = RepositoryId::from_uuid(Uuid::from_u128(999));
        let err = store
            .execute_source(
                &source_id.to_string(),
                SourceCommandEnvelope {
                    meta: meta(3),
                    command: SourceCommand::LinkRepository {
                        source_id,
                        repo_ref: RepoRef {
                            repository_id: missing_repository,
                            call_number: None,
                            media_type: SourceMediaType::Book,
                        },
                    },
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, CommandError::Rejected(SourceError::UnknownRepository(r)) if r == missing_repository),
            "expected UnknownRepository, got {err:?}"
        );
    }

    #[tokio::test]
    async fn source_linking_a_present_repository_succeeds_and_projects() {
        use genealogy_core::enums::SourceMediaType;
        use genealogy_core::ids::{RepositoryId, SourceId};
        use genealogy_core::repo_ref::RepoRef;
        use genealogy_core::repository::command::{RepositoryCommand, RepositoryCommandEnvelope};
        use genealogy_core::source::command::{SourceCommand, SourceCommandEnvelope};

        let (store, _dir) = store().await;
        // Create the repository first; its projection commits before the source links it, so the
        // resolver sees it.
        let repository_id = RepositoryId::from_uuid(Uuid::from_u128(1));
        store
            .execute_repository(
                &repository_id.to_string(),
                RepositoryCommandEnvelope {
                    meta: meta(2),
                    command: RepositoryCommand::CreateRepository {
                        repository_id,
                        human_id: HumanId::new("R0001"),
                    },
                },
            )
            .await
            .unwrap();

        let source_id = SourceId::from_uuid(Uuid::from_u128(2));
        for command in [
            SourceCommand::CreateSource {
                source_id,
                human_id: HumanId::new("S0001"),
            },
            SourceCommand::LinkRepository {
                source_id,
                repo_ref: RepoRef {
                    repository_id,
                    call_number: Some("MS 1234".to_owned()),
                    media_type: SourceMediaType::Manuscript,
                },
            },
        ] {
            store
                .execute_source(&source_id.to_string(), SourceCommandEnvelope { meta: meta(3), command })
                .await
                .unwrap();
        }

        let view = store.find_source("S0001").await.unwrap().expect("source projected");
        assert_eq!(
            view.repositories(),
            vec![&RepoRef {
                repository_id,
                call_number: Some("MS 1234".to_owned()),
                media_type: SourceMediaType::Manuscript,
            }]
        );
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

    /// Snapshots every projection table as ordered `(table, view_id, version, payload)` rows, so two
    /// snapshots can be compared for byte-exact equality.
    async fn dump_all_views(store: &SqliteStore) -> Vec<(String, String, i64, String)> {
        let mut rows = Vec::new();
        for &table in super::ALL_VIEW_TABLES {
            let fetched = sqlx::query(&format!(
                "SELECT view_id, version, payload FROM {table} ORDER BY view_id"
            ))
            .fetch_all(&store.pool)
            .await
            .unwrap();
            for row in fetched {
                rows.push((
                    table.to_owned(),
                    row.get("view_id"),
                    row.get("version"),
                    row.get("payload"),
                ));
            }
        }
        rows
    }

    #[tokio::test]
    async fn rebuild_reproduces_identical_projections() {
        use genealogy_core::citation::command::{CitationCommand, CitationCommandEnvelope};
        use genealogy_core::enums::{EventType, PlaceType};
        use genealogy_core::event::command::{EventCommand, EventCommandEnvelope};
        use genealogy_core::family::command::{FamilyCommand, FamilyCommandEnvelope};
        use genealogy_core::ids::{CitationId, EventId, FamilyId, PlaceId, SourceId};
        use genealogy_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
        use genealogy_core::source::command::{SourceCommand, SourceCommandEnvelope};

        let (store, _dir) = store().await;
        // A small dataset spanning several aggregates, including the cross-aggregate Event→Place link.
        let family_id = FamilyId::from_uuid(Uuid::from_u128(1));
        store
            .execute_family(
                &family_id.to_string(),
                FamilyCommandEnvelope {
                    meta: meta(2),
                    command: FamilyCommand::CreateFamily {
                        family_id,
                        human_id: HumanId::new("F0001"),
                    },
                },
            )
            .await
            .unwrap();
        let place_id = PlaceId::from_uuid(Uuid::from_u128(2));
        store
            .execute_place(
                &place_id.to_string(),
                PlaceCommandEnvelope {
                    meta: meta(3),
                    command: PlaceCommand::CreatePlace {
                        place_id,
                        human_id: HumanId::new("P0001"),
                        place_type: PlaceType::Parish,
                    },
                },
            )
            .await
            .unwrap();
        let event_id = EventId::from_uuid(Uuid::from_u128(3));
        for command in [
            EventCommand::CreateEvent {
                event_id,
                human_id: HumanId::new("E0001"),
                event_type: EventType::Birth,
            },
            EventCommand::LinkPlace { event_id, place_id },
        ] {
            store
                .execute_event(&event_id.to_string(), EventCommandEnvelope { meta: meta(4), command })
                .await
                .unwrap();
        }
        let source_id = SourceId::from_uuid(Uuid::from_u128(4));
        store
            .execute_source(
                &source_id.to_string(),
                SourceCommandEnvelope {
                    meta: meta(5),
                    command: SourceCommand::CreateSource {
                        source_id,
                        human_id: HumanId::new("S0001"),
                    },
                },
            )
            .await
            .unwrap();
        let citation_id = CitationId::from_uuid(Uuid::from_u128(5));
        store
            .execute_citation(
                &citation_id.to_string(),
                CitationCommandEnvelope {
                    meta: meta(6),
                    command: CitationCommand::CreateCitation {
                        citation_id,
                        human_id: HumanId::new("C0001"),
                        source_id,
                    },
                },
            )
            .await
            .unwrap();

        let before = dump_all_views(&store).await;
        assert!(!before.is_empty(), "dataset should project some views");
        store.rebuild_projections().await.unwrap();
        let after = dump_all_views(&store).await;
        assert_eq!(before, after, "rebuild must reproduce identical projections");
    }

    #[tokio::test]
    async fn repository_projects_and_rebuilds_identically() {
        use genealogy_core::address::Address;
        use genealogy_core::enums::RepositoryType;
        use genealogy_core::ids::RepositoryId;
        use genealogy_core::repository::command::{RepositoryCommand, RepositoryCommandEnvelope};

        let (store, _dir) = store().await;
        let repository_id = RepositoryId::from_uuid(Uuid::from_u128(1));
        for command in [
            RepositoryCommand::CreateRepository {
                repository_id,
                human_id: HumanId::new("R0001"),
            },
            RepositoryCommand::SetName {
                repository_id,
                name: "Riksarkivet".to_owned(),
            },
            RepositoryCommand::SetRepositoryType {
                repository_id,
                repository_type: RepositoryType::Archive,
            },
            RepositoryCommand::AddAddress {
                repository_id,
                address: Address {
                    locality: Some("Oslo".to_owned()),
                    country: Some("Norway".to_owned()),
                    ..Address::default()
                },
            },
        ] {
            store
                .execute_repository(
                    &repository_id.to_string(),
                    RepositoryCommandEnvelope { meta: meta(2), command },
                )
                .await
                .unwrap();
        }

        let view = store
            .find_repository("R0001")
            .await
            .unwrap()
            .expect("repository projected");
        assert_eq!(view.name(), Some("Riksarkivet"));
        assert_eq!(view.repository_type(), Some(&RepositoryType::Archive));
        assert_eq!(view.addresses().len(), 1);

        let before = dump_all_views(&store).await;
        store.rebuild_projections().await.unwrap();
        let after = dump_all_views(&store).await;
        assert_eq!(before, after, "rebuild must reproduce identical projections");
    }

    #[tokio::test]
    async fn place_enclosed_by_a_missing_place_is_rejected_through_services() {
        use genealogy_core::enums::PlaceType;
        use genealogy_core::ids::PlaceId;
        use genealogy_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
        use genealogy_core::place::error::PlaceError;
        use genealogy_core::place_ref::PlaceRef;

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
                        place_type: PlaceType::Farm,
                    },
                },
            )
            .await
            .unwrap();

        // No enclosing place exists, so the resolver reports it absent and `decide` rejects — the
        // aggregate-tax path (ADR 0004 §3), symmetric with Event→Place.
        let missing = PlaceId::from_uuid(Uuid::from_u128(999));
        let err = store
            .execute_place(
                &place_id.to_string(),
                PlaceCommandEnvelope {
                    meta: meta(3),
                    command: PlaceCommand::AssertEnclosedBy {
                        place_id,
                        enclosed_by: PlaceRef {
                            place_id: missing,
                            date: None,
                        },
                    },
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, CommandError::Rejected(PlaceError::UnknownPlace(p)) if p == missing),
            "expected UnknownPlace, got {err:?}"
        );
    }

    #[tokio::test]
    async fn place_enclosure_coordinates_and_code_project_and_rebuild_identically() {
        use genealogy_core::enums::PlaceType;
        use genealogy_core::geo::{GeoCoordinates, Microdegrees};
        use genealogy_core::ids::PlaceId;
        use genealogy_core::place::command::{PlaceCommand, PlaceCommandEnvelope};
        use genealogy_core::place_ref::PlaceRef;

        let (store, _dir) = store().await;
        let parish = PlaceId::from_uuid(Uuid::from_u128(1));
        let farm = PlaceId::from_uuid(Uuid::from_u128(2));
        for (id, human, ty) in [(parish, "P0001", PlaceType::Parish), (farm, "P0002", PlaceType::Farm)] {
            store
                .execute_place(
                    &id.to_string(),
                    PlaceCommandEnvelope {
                        meta: meta(3),
                        command: PlaceCommand::CreatePlace {
                            place_id: id,
                            human_id: HumanId::new(human),
                            place_type: ty,
                        },
                    },
                )
                .await
                .unwrap();
        }
        // Enclose the farm in the parish (both projected), and set its decimal coordinates + code.
        for command in [
            PlaceCommand::AssertEnclosedBy {
                place_id: farm,
                enclosed_by: PlaceRef {
                    place_id: parish,
                    date: None,
                },
            },
            PlaceCommand::AssertCoordinates {
                place_id: farm,
                coordinates: GeoCoordinates {
                    latitude: Microdegrees::from_microdegrees(61_877_500),
                    longitude: Microdegrees::from_microdegrees(9_098_900),
                },
            },
            PlaceCommand::SetCode {
                place_id: farm,
                code: "0515".to_owned(),
            },
        ] {
            store
                .execute_place(&farm.to_string(), PlaceCommandEnvelope { meta: meta(4), command })
                .await
                .unwrap();
        }

        let view = store.find_place("P0002").await.unwrap().expect("place projected");
        assert_eq!(view.code(), Some("0515"));
        assert_eq!(view.enclosed_by().len(), 1);
        assert_eq!(
            view.coordinates().map(|c| c.latitude.as_microdegrees()),
            Some(61_877_500)
        );

        // The fixed-point coordinates survive a byte-exact projection rebuild (the reason decimals
        // are scaled integers, not f64 — data-model §15 note).
        let before = dump_all_views(&store).await;
        store.rebuild_projections().await.unwrap();
        let after = dump_all_views(&store).await;
        assert_eq!(before, after, "rebuild must reproduce identical projections");
    }

    #[tokio::test]
    async fn dna_test_for_a_missing_person_is_rejected_through_services() {
        use genealogy_core::dna_test::command::{DnaTestCommand, DnaTestCommandEnvelope};
        use genealogy_core::dna_test::error::DnaTestError;
        use genealogy_core::ids::{DnaTestId, PersonId};

        let (store, _dir) = store().await;
        // No person exists, so the resolver reports it absent and `decide` rejects — the aggregate
        // tax path (ADR 0004 §3).
        let dna_test_id = DnaTestId::from_uuid(Uuid::from_u128(1));
        let missing_person = PersonId::from_uuid(Uuid::from_u128(999));
        let err = store
            .execute_dna_test(
                &dna_test_id.to_string(),
                DnaTestCommandEnvelope {
                    meta: meta(2),
                    command: DnaTestCommand::CreateDnaTest {
                        dna_test_id,
                        human_id: HumanId::new("D0001"),
                        person_id: missing_person,
                    },
                },
            )
            .await
            .unwrap_err();
        assert!(
            matches!(err, CommandError::Rejected(DnaTestError::UnknownPerson(p)) if p == missing_person),
            "expected UnknownPerson, got {err:?}"
        );
    }

    #[tokio::test]
    async fn dna_test_for_a_present_person_projects_and_rebuilds_identically() {
        use genealogy_core::dna::{DnaProvider, DnaTestType};
        use genealogy_core::dna_test::command::{DnaTestCommand, DnaTestCommandEnvelope};
        use genealogy_core::enums::EvidenceLevel;
        use genealogy_core::ids::{DnaTestId, PersonId};

        let (store, _dir) = store().await;
        let person_id = PersonId::from_uuid(Uuid::from_u128(1));
        store
            .execute_person(
                &person_id.to_string(),
                PersonCommandEnvelope {
                    meta: meta(2),
                    command: PersonCommand::CreatePerson {
                        person_id,
                        human_id: HumanId::new("I0001"),
                        evidence_level: EvidenceLevel::Conclusion,
                    },
                },
            )
            .await
            .unwrap();

        let dna_test_id = DnaTestId::from_uuid(Uuid::from_u128(2));
        for command in [
            DnaTestCommand::CreateDnaTest {
                dna_test_id,
                human_id: HumanId::new("D0001"),
                person_id,
            },
            DnaTestCommand::SetProvider {
                dna_test_id,
                provider: DnaProvider::MyHeritage,
            },
            DnaTestCommand::SetTestType {
                dna_test_id,
                test_type: DnaTestType::Autosomal,
            },
            DnaTestCommand::AssertHaplogroup {
                dna_test_id,
                haplogroup: "R-M269".to_owned(),
            },
        ] {
            store
                .execute_dna_test(
                    &dna_test_id.to_string(),
                    DnaTestCommandEnvelope { meta: meta(3), command },
                )
                .await
                .unwrap();
        }

        let view = store.find_dna_test("D0001").await.unwrap().expect("dna test projected");
        assert_eq!(view.person_id(), Some(person_id));
        assert_eq!(view.test_type(), Some(DnaTestType::Autosomal));
        assert_eq!(view.haplogroups().len(), 1);

        let before = dump_all_views(&store).await;
        store.rebuild_projections().await.unwrap();
        let after = dump_all_views(&store).await;
        assert_eq!(before, after, "rebuild must reproduce identical projections");
    }

    #[tokio::test]
    async fn dna_match_between_present_tests_projects_and_rebuilds_with_fixed_point_cm() {
        use genealogy_core::dna::{Centimorgans, DnaProvider};
        use genealogy_core::dna_match::command::{DnaMatchCommand, DnaMatchCommandEnvelope};
        use genealogy_core::dna_match::error::DnaMatchError;
        use genealogy_core::dna_test::command::{DnaTestCommand, DnaTestCommandEnvelope};
        use genealogy_core::enums::EvidenceLevel;
        use genealogy_core::ids::{DnaMatchId, DnaTestId, PersonId};

        let (store, _dir) = store().await;
        // Two persons, each with a test, so both sides of the match resolve.
        let mut tests = Vec::new();
        for (n, person_human, test_human) in [(1u128, "I0001", "D0001"), (2, "I0002", "D0002")] {
            let person_id = PersonId::from_uuid(Uuid::from_u128(n));
            store
                .execute_person(
                    &person_id.to_string(),
                    PersonCommandEnvelope {
                        meta: meta(2),
                        command: PersonCommand::CreatePerson {
                            person_id,
                            human_id: HumanId::new(person_human),
                            evidence_level: EvidenceLevel::Conclusion,
                        },
                    },
                )
                .await
                .unwrap();
            let test_id = DnaTestId::from_uuid(Uuid::from_u128(n + 100));
            store
                .execute_dna_test(
                    &test_id.to_string(),
                    DnaTestCommandEnvelope {
                        meta: meta(3),
                        command: DnaTestCommand::CreateDnaTest {
                            dna_test_id: test_id,
                            human_id: HumanId::new(test_human),
                            person_id,
                        },
                    },
                )
                .await
                .unwrap();
            tests.push(test_id);
        }

        // A match against a missing test is rejected through the resolver.
        let dna_match_id = DnaMatchId::from_uuid(Uuid::from_u128(9));
        let missing = DnaTestId::from_uuid(Uuid::from_u128(999));
        let err = store
            .execute_dna_match(
                &dna_match_id.to_string(),
                DnaMatchCommandEnvelope {
                    meta: meta(4),
                    command: DnaMatchCommand::ObserveMatch {
                        dna_match_id,
                        human_id: HumanId::new("X0001"),
                        test_a: tests[0],
                        test_b: missing,
                        provider: DnaProvider::MyHeritage,
                        shared_cm: Centimorgans::from_hundredths(85_050),
                        percent_shared: None,
                        segment_count: 3,
                        largest_segment_cm: Centimorgans::from_hundredths(4500),
                        predicted_relationship: None,
                    },
                },
            )
            .await
            .unwrap_err();
        assert!(matches!(err, CommandError::Rejected(DnaMatchError::UnknownTest(t)) if t == missing));

        // A match between the two present tests is accepted and projects.
        store
            .execute_dna_match(
                &dna_match_id.to_string(),
                DnaMatchCommandEnvelope {
                    meta: meta(5),
                    command: DnaMatchCommand::ObserveMatch {
                        dna_match_id,
                        human_id: HumanId::new("X0001"),
                        test_a: tests[0],
                        test_b: tests[1],
                        provider: DnaProvider::MyHeritage,
                        shared_cm: Centimorgans::from_hundredths(85_050),
                        percent_shared: None,
                        segment_count: 3,
                        largest_segment_cm: Centimorgans::from_hundredths(4500),
                        predicted_relationship: Some("2nd cousin".to_owned()),
                    },
                },
            )
            .await
            .unwrap();

        let view = store
            .find_dna_match("X0001")
            .await
            .unwrap()
            .expect("dna match projected");
        assert_eq!(view.shared_cm().map(Centimorgans::as_hundredths), Some(85_050));

        // The fixed-point cM survives a byte-exact projection rebuild (the reason cM is a scaled int).
        let before = dump_all_views(&store).await;
        store.rebuild_projections().await.unwrap();
        let after = dump_all_views(&store).await;
        assert_eq!(before, after, "rebuild must reproduce identical projections");
    }

    #[tokio::test]
    async fn tag_projects_by_id_and_rebuilds_identically() {
        use genealogy_core::ids::TagId;
        use genealogy_core::tag::command::{TagCommand, TagCommandEnvelope};

        let (store, _dir) = store().await;
        let tag_id = TagId::from_uuid(Uuid::from_u128(1));
        for command in [
            TagCommand::CreateTag {
                tag_id,
                name: "Direct line".to_owned(),
            },
            TagCommand::SetTagColor {
                tag_id,
                color: "#1f77b4".to_owned(),
            },
            TagCommand::SetTagPriority { tag_id, priority: 5 },
        ] {
            store
                .execute_tag(&tag_id.to_string(), TagCommandEnvelope { meta: meta(2), command })
                .await
                .unwrap();
        }

        // Tags have no human_id; they are looked up by their aggregate id.
        let view = store
            .find_tag(&tag_id.to_string())
            .await
            .unwrap()
            .expect("tag projected");
        assert_eq!(view.name(), Some("Direct line"));
        assert_eq!(view.color(), Some("#1f77b4"));
        assert_eq!(view.priority(), Some(5));

        let before = dump_all_views(&store).await;
        store.rebuild_projections().await.unwrap();
        let after = dump_all_views(&store).await;
        assert_eq!(before, after, "rebuild must reproduce identical projections");
    }
}
