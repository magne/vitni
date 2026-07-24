//! Postgres backend for [`Store`](crate::store::Store) — private wiring (ADR 0002).
//!
//! The server twin of [`sqlite`](crate::sqlite): one `cqrs-es` framework per aggregate over the
//! `postgres-es` repositories, sharing the read-model pool. The per-aggregate fields, wiring,
//! methods, and rebuild are generated from the [`registry`](crate::registry), identically to the
//! SQLite backend. Everything here (`sqlx`, `postgres-es`, `cqrs-es`) is an implementation detail;
//! only [`crate::store`] re-exposes it, in engine-neutral terms.

use std::sync::Arc;
use std::time::Duration;

use cqrs_es::persist::{EventUpcaster, GenericQuery, PersistedEventStore, QueryReplay};
use cqrs_es::{Aggregate, CqrsFramework, View};
use postgres_es::{PostgresEventRepository, PostgresViewRepository, postgres_cqrs};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

use crate::postgres_query;
use crate::registry::{for_each_db_aggregate, for_each_db_external_id_aggregate, for_each_db_human_id_aggregate};
use crate::resolver::PostgresRefStore;
use crate::schema;
use crate::store::{CommandError, DbError, map_aggregate_error};
use crate::tables::{
    ALL_VIEW_TABLES, CITATION_VIEW_TABLE, DNA_MATCH_VIEW_TABLE, DNA_TEST_VIEW_TABLE, EVENT_VIEW_TABLE,
    FAMILY_VIEW_TABLE, MEDIA_VIEW_TABLE, NOTE_VIEW_TABLE, PERSON_VIEW_TABLE, PLACE_VIEW_TABLE, REPOSITORY_VIEW_TABLE,
    RESEARCH_NOTE_VIEW_TABLE, SOURCE_VIEW_TABLE, TAG_VIEW_TABLE,
};

/// The default pool size for a Postgres workspace connection.
const MAX_CONNECTIONS: u32 = 10;

/// How long `open()` waits for the first connection before reporting the server unreachable. Kept
/// short so a misconfigured `database_url` fails fast rather than hanging on the 30 s pool default.
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);

/// Builds one aggregate's `CqrsFramework` in `open()`, matching the registry `wiring` column: a
/// plain unit `Services`, a projection-reading resolver (the §9 aggregate tax), or the
/// hand-assembled Event store that carries upcasters at load (ADR 0010).
macro_rules! postgres_open_cqrs {
    ($pool:ident, $repo:ident, (plain)) => {
        postgres_cqrs($pool.clone(), vec![Box::new(GenericQuery::new($repo))], ())
    };
    ($pool:ident, $repo:ident, (resolver $resolver:path)) => {
        postgres_cqrs(
            $pool.clone(),
            vec![Box::new(GenericQuery::new($repo))],
            <$resolver>::new(PostgresRefStore::shared($pool.clone())),
        )
    };
    ($pool:ident, $repo:ident, (event $resolver:path)) => {{
        let store = PersistedEventStore::new_event_store(PostgresEventRepository::new($pool.clone()))
            .with_upcasters(genealogy_core::event::upcasters());
        CqrsFramework::new(
            store,
            vec![Box::new(GenericQuery::new($repo))],
            <$resolver>::new(PostgresRefStore::shared($pool.clone())),
        )
    }};
}

/// Selects the read-model lookup for `find_*`, keyed by the registry `find_param` column: Tag is
/// keyed by its own id (`find_view_by_id`), every other aggregate by its `human_id`.
macro_rules! postgres_find_query {
    ($pool:expr, $table:expr, human_id, $value:expr) => {
        postgres_query::find_view_by_human_id($pool, $table, $value)
    };
    ($pool:expr, $table:expr, tag_id, $value:expr) => {
        postgres_query::find_view_by_id($pool, $table, $value)
    };
}

/// Generates the Postgres backend from the registry: the per-aggregate `CqrsFramework` fields,
/// `open()` wiring, the command/find/list methods, and the rebuild loop. The projection-table
/// constants come from [`crate::tables`].
macro_rules! postgres_store {
    ($(($snake:ident, $State:ty, $View:ty, $Cmd:ty, $Err:ty, $table_const:ident, $table_str:literal, $execute:ident, $find:ident, $find_param:ident, $list:ident, $wiring:tt, $upcasters:expr,)),+ $(,)?) => {
        /// A Postgres-backed store: one command framework per aggregate, sharing the read-model pool.
        pub(crate) struct PostgresStore {
            $(
                $snake: CqrsFramework<$State, PersistedEventStore<PostgresEventRepository, $State>>,
            )+
            pool: Pool<Postgres>,
        }

        impl PostgresStore {
            /// Connects the pool for `database_url`, runs the (idempotent) DDL, and wires the projections.
            pub(crate) async fn open(database_url: &str) -> Result<Self, DbError> {
                let pool = PgPoolOptions::new()
                    .max_connections(MAX_CONNECTIONS)
                    .acquire_timeout(CONNECT_TIMEOUT)
                    .connect(database_url)
                    .await
                    .map_err(|e| DbError::Backend(format!("connecting to postgres: {e}")))?;
                schema::init_postgres(&pool)
                    .await
                    .map_err(|e| DbError::Backend(format!("initializing event store: {e}")))?;
                for &table in ALL_VIEW_TABLES {
                    schema::create_postgres_view_table(&pool, table)
                        .await
                        .map_err(|e| DbError::Backend(format!("creating projection table {table}: {e}")))?;
                }
                $(
                    let repo = Arc::new(PostgresViewRepository::<$View, $State>::new($table_const, pool.clone()));
                    let $snake = postgres_open_cqrs!(pool, repo, $wiring);
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
                    postgres_find_query!(&self.pool, $table_const, $find_param, $find_param).await
                }

                pub(crate) async fn $list(&self) -> Result<Vec<$View>, DbError> {
                    postgres_query::list_views(&self.pool, $table_const).await
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

for_each_db_aggregate!(postgres_store);

/// Generates the per-aggregate `next_*_human_id` allocators (every aggregate but Tag).
macro_rules! postgres_next_methods {
    ($(($snake:ident, $next:ident, $table_const:ident)),+ $(,)?) => {
        impl PostgresStore {
            $(
                pub(crate) async fn $next(&self, format: &genealogy_core::id_format::IdFormat) -> Result<String, DbError> {
                    postgres_query::next_human_id(&self.pool, $table_const, format).await
                }
            )+
        }
    };
}

for_each_db_human_id_aggregate!(postgres_next_methods);

/// Generates the per-aggregate `find_*_by_external_id` lookups for the aggregates that carry
/// external ids (data-model §11).
macro_rules! postgres_external_id_methods {
    ($(($snake:ident, $find:ident, $table_const:ident, $View:ty)),+ $(,)?) => {
        impl PostgresStore {
            $(
                pub(crate) async fn $find(&self, authority: &str, value: &str) -> Result<Option<$View>, DbError> {
                    postgres_query::find_view_by_external_id(&self.pool, $table_const, authority, value).await
                }
            )+
        }
    };
}

for_each_db_external_id_aggregate!(postgres_external_id_methods);

/// The change-log / count read path (Phase 5 PR 5): the Postgres twin of the SQLite backend's
/// hand-written raw-event and aggregate-count reads.
impl PostgresStore {
    pub(crate) async fn read_aggregate_events(
        &self,
        aggregate_type: &str,
        aggregate_id: &str,
    ) -> Result<Vec<crate::store::StoredEvent>, DbError> {
        postgres_query::read_aggregate_events(&self.pool, aggregate_type, aggregate_id).await
    }

    pub(crate) async fn read_recent_events(&self, limit: u32) -> Result<Vec<crate::store::StoredEvent>, DbError> {
        postgres_query::read_recent_events(&self.pool, limit).await
    }

    pub(crate) async fn human_id_index(&self, table: &str) -> Result<Vec<(String, String)>, DbError> {
        postgres_query::human_id_index(&self.pool, table).await
    }

    pub(crate) async fn count(&self, table: &str) -> Result<u64, DbError> {
        postgres_query::count_rows(&self.pool, table).await
    }

    /// Every research note whose `subjects` set names the subject serialized under `subject_kind`
    /// (`Person`/`Family`/`Event`/`Place`) — the Postgres twin of
    /// [`crate::sqlite::SqliteStore::list_research_notes_for_subject`] (ADR 0028 §5).
    pub(crate) async fn list_research_notes_for_subject(
        &self,
        subject_kind: &str,
        subject_value: &str,
    ) -> Result<Vec<genealogy_core::research_note::ResearchNoteView>, DbError> {
        postgres_query::list_views_by_subject(&self.pool, RESEARCH_NOTE_VIEW_TABLE, subject_kind, subject_value).await
    }
}

/// Clears one view table and replays its aggregate's full event log back into it (ADR 0010).
///
/// `upcasters` migrate historical payloads during the replay; pass an empty vec for aggregates
/// whose schema has not evolved. `stream_all_events::<A>()` binds the aggregate type, so each
/// replay sees only its own events.
async fn rebuild_view<A, V>(
    pool: &Pool<Postgres>,
    table: &str,
    upcasters: Vec<Box<dyn EventUpcaster>>,
) -> Result<(), DbError>
where
    A: Aggregate,
    V: View<A>,
{
    schema::clear_postgres_view_table(pool, table)
        .await
        .map_err(|e| DbError::Backend(format!("clearing projection {table}: {e}")))?;
    let repo = Arc::new(PostgresViewRepository::<V, A>::new(table, pool.clone()));
    let replay =
        QueryReplay::new(PostgresEventRepository::new(pool.clone()), GenericQuery::new(repo)).with_upcasters(upcasters);
    replay
        .replay_all()
        .await
        .map_err(|e| DbError::Backend(format!("rebuilding projection {table}: {e}")))
}
