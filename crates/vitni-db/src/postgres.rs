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
use cqrs_es::{Aggregate, AggregateContext as _, CqrsFramework, EventStore as _, View};
use postgres_es::{PostgresEventRepository, PostgresViewRepository, postgres_cqrs};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

use crate::place_succession_index;
use crate::postgres_query;
use crate::registry::{for_each_db_aggregate, for_each_db_external_id_aggregate, for_each_db_human_id_aggregate};
use crate::resolver::PostgresRefStore;
use crate::schema;
use crate::store::{CommandError, DbError, map_aggregate_error};
use crate::tables::{
    ALL_VIEW_TABLES, CITATION_VIEW_TABLE, DNA_MATCH_VIEW_TABLE, DNA_TEST_VIEW_TABLE, EVENT_VIEW_TABLE,
    FAMILY_VIEW_TABLE, HUMAN_ID_VIEW_TABLES, IMPORT_RUN_VIEW_TABLE, MEDIA_VIEW_TABLE, NOTE_VIEW_TABLE,
    PERSON_VIEW_TABLE, PLACE_VIEW_TABLE, REPOSITORY_VIEW_TABLE, RESEARCH_NOTE_VIEW_TABLE, SOURCE_VIEW_TABLE,
    TAG_VIEW_TABLE,
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
            .with_upcasters(vitni_core::event::upcasters());
        CqrsFramework::new(
            store,
            vec![Box::new(GenericQuery::new($repo))],
            <$resolver>::new(PostgresRefStore::shared($pool.clone())),
        )
    }};
}

/// Appends the derived side indexes to the one framework each is fed by: the identity cluster index
/// (ADR 0039 §4) to every matchable kind but `tag`, and the succession index (ADR 0026 §4) to `place`,
/// leaving every other aggregate's framework untouched. Dispatches on the registry's literal `$snake`
/// token — the same "wiring by tag" shape as [`postgres_open_cqrs!`] — rather than naming the aggregate
/// after the per-aggregate repetition, which a plain `let` can't see across macro hygiene.
///
/// The SQLite twin also appends a geometry index; that one is `geo-types`/`geozero`-backed and
/// sqlite-only, so the Postgres mirror is a separate follow-up (ADR 0024 §3).
macro_rules! postgres_wire_side_indexes {
    (person, $pool:expr, $framework:expr) => {
        $framework.append_query(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::person::PersonView,
        >::new($pool.clone())))
    };
    (event, $pool:expr, $framework:expr) => {
        $framework.append_query(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::event::EventView,
        >::new($pool.clone())))
    };
    (family, $pool:expr, $framework:expr) => {
        $framework.append_query(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::family::FamilyView,
        >::new($pool.clone())))
    };
    (place, $pool:expr, $framework:expr) => {
        $framework
            .append_query(Box::new(
                place_succession_index::postgres::PlaceSuccessionIndexQuery::new($pool.clone()),
            ))
            .append_query(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
                vitni_core::place::PlaceView,
            >::new($pool.clone())))
    };
    (source, $pool:expr, $framework:expr) => {
        $framework.append_query(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::source::SourceView,
        >::new($pool.clone())))
    };
    (citation, $pool:expr, $framework:expr) => {
        $framework.append_query(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::citation::CitationView,
        >::new($pool.clone())))
    };
    (repository, $pool:expr, $framework:expr) => {
        $framework.append_query(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::repository::RepositoryView,
        >::new($pool.clone())))
    };
    (note, $pool:expr, $framework:expr) => {
        $framework.append_query(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::note::NoteView,
        >::new($pool.clone())))
    };
    (media, $pool:expr, $framework:expr) => {
        $framework.append_query(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::media::MediaView,
        >::new($pool.clone())))
    };
    ($other:ident, $pool:expr, $framework:expr) => {
        $framework
    };
}

/// The `Services` an aggregate's `handle` needs outside its framework, matching the registry
/// `wiring` column — for [`PostgresStore`]'s previews.
macro_rules! postgres_services {
    ($pool:expr, (plain)) => {
        ()
    };
    ($pool:expr, (resolver $resolver:path)) => {
        <$resolver>::new(PostgresRefStore::shared($pool.clone()))
    };
    ($pool:expr, (event $resolver:path)) => {
        <$resolver>::new(PostgresRefStore::shared($pool.clone()))
    };
}

/// Selects the read-model lookup for `find_*`, keyed by the registry `find_param` column: Tag and
/// `ImportRun` are keyed by their own id (`find_view_by_id`), every other aggregate by its `human_id`.
macro_rules! postgres_find_query {
    ($pool:expr, $table:expr, human_id, $value:expr) => {
        postgres_query::find_view_by_human_id($pool, $table, $value)
    };
    ($pool:expr, $table:expr, tag_id, $value:expr) => {
        postgres_query::find_view_by_id($pool, $table, $value)
    };
    ($pool:expr, $table:expr, run_id, $value:expr) => {
        postgres_query::find_view_by_id($pool, $table, $value)
    };
}

/// Generates the Postgres backend from the registry: the per-aggregate `CqrsFramework` fields,
/// `open()` wiring, the command/find/list methods, and the rebuild loop. The projection-table
/// constants come from [`crate::tables`].
macro_rules! postgres_store {
    ($(($snake:ident, $State:ty, $View:ty, $Cmd:ty, $Err:ty, $table_const:ident, $table_str:literal, $execute:ident, $find:ident, $find_param:ident, $list:ident, $preview:ident, $wiring:tt, $upcasters:expr,)),+ $(,)?) => {
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
                // Drops and recreates any view table left over from before the `human_id` column
                // existed (ADR 0032), creates every table fresh, and indexes the human-id-bearing
                // ones; returns which tables were dropped, so their aggregate's event log can be
                // replayed to repopulate them below (rebuilding needs the aggregate's `State`/`View`
                // types, which this untyped helper does not have).
                let stale_tables = migrate_postgres_view_tables(&pool).await?;
                // The Place succession cross-reference index (ADR 0026 §4) is not one of the generic
                // per-aggregate projections: it is derived from the Place projection and keyed by its
                // own tables. Its DDL is created up front, and its `Query` is appended only to the
                // Place framework below (`postgres_wire_side_indexes!`).
                place_succession_index::postgres::create_tables(&pool)
                    .await
                    .map_err(|e| DbError::Backend(format!("creating place succession index: {e}")))?;
                // The record origins index (ADR 0037 §4) is fed by every aggregate. A workspace
                // whose log predates it gets it filled from that log below.
                // The match keys blocking index (ADR 0038 §7): every commit marks the matchable
                // aggregate it touched, and the app layer rekeys it before the next lookup.
                crate::match_keys::postgres::create_tables(&pool)
                    .await
                    .map_err(|e| DbError::Backend(format!("creating match keys index: {e}")))?;
                // The identity cluster index (ADR 0039 §4) is derived from the projections of every
                // matchable kind; its `Query` is appended to those frameworks below, and a workspace that
                // predates it gets it filled once the projections are open.
                let identity_is_new = crate::identity_links::postgres::create_tables(&pool)
                    .await
                    .map_err(|e| DbError::Backend(format!("creating identity index: {e}")))?;
                let origins_are_new = crate::record_origins::postgres::create_tables(&pool)
                    .await
                    .map_err(|e| DbError::Backend(format!("creating record origins index: {e}")))?;
                $(
                    let repo = Arc::new(PostgresViewRepository::<$View, $State>::new($table_const, pool.clone()));
                    let $snake = postgres_open_cqrs!(pool, repo, $wiring);
                    let $snake = postgres_wire_side_indexes!($snake, pool, $snake).append_query(Box::new(
                        crate::record_origins::postgres::RecordOriginsQuery::new(pool.clone(), $table_const),
                    )).append_query(Box::new(crate::match_keys::postgres::MatchDirtyQuery::new(pool.clone())));
                    if stale_tables.contains(&$table_const) {
                        tracing::info!(
                            table = $table_const,
                            "projection table predated the human_id column; rebuilding from the event log"
                        );
                        rebuild_view::<$State, $View>(&pool, $table_const, $upcasters).await?;
                    }
                    if origins_are_new {
                        replay_record_origins::<$State>(&pool, $table_const, $upcasters).await?;
                    }
                )+
                if identity_is_new {
                    crate::identity_links::postgres::rebuild_index(&pool).await?;
                }
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

                /// The events `command` would emit against the aggregate's current state, without
                /// committing them: the aggregate is loaded and handled exactly as `execute` does.
                pub(crate) async fn $preview(
                    &self,
                    aggregate_id: &str,
                    command: $Cmd,
                ) -> Result<Vec<<$State as Aggregate>::Event>, CommandError<$Err>> {
                    let store = PersistedEventStore::<PostgresEventRepository, $State>::new_event_store(PostgresEventRepository::new(self.pool.clone()))
                        .with_upcasters($upcasters);
                    let mut context = store.load_aggregate(aggregate_id).await.map_err(map_aggregate_error)?;
                    let services: <$State as Aggregate>::Services = postgres_services!(self.pool, $wiring);
                    let sink = cqrs_es::event_sink::EventSink::default();
                    context
                        .aggregate()
                        .handle(command, &services, &sink)
                        .await
                        .map_err(CommandError::Rejected)?;
                    Ok(sink.collect().await)
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
                // Place's succession cross-reference index (ADR 0026 §4) is derived from the (now
                // freshly rebuilt) Place projection above, not replayed from raw events itself.
                place_succession_index::postgres::rebuild_index(&self.pool).await?;
                // The identity clusters (ADR 0039 §4) are derived from the rebuilt projections of every
                // matchable kind.
                crate::identity_links::postgres::rebuild_index(&self.pool).await?;
                // The record origins index is replayed from the raw events, after the projections
                // its `live` flags are read from.
                crate::record_origins::postgres::clear_table(&self.pool).await?;
                // The match keys need the name-culture packs, which only the app layer has: forgetting
                // what the index was built under makes it rebuild on next use.
                crate::match_keys::postgres::clear_state(&self.pool).await?;
                $(
                    replay_record_origins::<$State>(&self.pool, $table_const, $upcasters).await?;
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
                pub(crate) async fn $next(&self, format: &vitni_core::id_format::IdFormat) -> Result<String, DbError> {
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

    pub(crate) async fn read_raw_events(
        &self,
        after: Option<&crate::raw::RawEventKey>,
        limit: u32,
    ) -> Result<Vec<crate::raw::RawEvent>, DbError> {
        postgres_query::read_raw_events(&self.pool, after, limit).await
    }

    pub(crate) async fn insert_raw_events(
        &self,
        rows: impl IntoIterator<Item = Result<crate::raw::RawEvent, DbError>>,
    ) -> Result<u64, DbError> {
        postgres_query::insert_raw_events(&self.pool, rows).await
    }

    pub(crate) async fn discard_all_events(&self) -> Result<(), DbError> {
        postgres_query::discard_all_events(&self.pool).await
    }

    pub(crate) async fn event_count(&self) -> Result<u64, DbError> {
        postgres_query::event_count(&self.pool).await
    }

    pub(crate) async fn projection_rows(&self) -> Result<Vec<crate::raw::ProjectionRow>, DbError> {
        let mut rows = Vec::new();
        for &table in ALL_VIEW_TABLES {
            rows.extend(postgres_query::projection_rows(&self.pool, table).await?);
        }
        Ok(rows)
    }

    pub(crate) async fn human_id_of(&self, table: &str, view_id: &str) -> Result<Option<String>, DbError> {
        postgres_query::human_id_of(&self.pool, table, view_id).await
    }

    pub(crate) async fn human_id_index(&self, table: &str) -> Result<Vec<(String, String)>, DbError> {
        postgres_query::human_id_index(&self.pool, table).await
    }

    pub(crate) async fn count(&self, table: &str) -> Result<u64, DbError> {
        postgres_query::count_rows(&self.pool, table).await
    }

    /// Every record origins row for `(dataset, record, item)` under `field_key` (ADR 0037 §4).
    pub(crate) async fn origin_rows(
        &self,
        dataset: &str,
        record: &str,
        item: Option<&str>,
        field_key: &str,
    ) -> Result<Vec<crate::record_origins::OriginRow>, DbError> {
        crate::record_origins::postgres::rows(&self.pool, dataset, record, item, field_key).await
    }

    /// The aggregate of `kind` that `(dataset, record, item)` resolves onto, if any (ADR 0037 §4).
    pub(crate) async fn resolve_origin(
        &self,
        dataset: &str,
        record: &str,
        item: Option<&str>,
        kind: &str,
    ) -> Result<Option<crate::record_origins::OriginResolution>, DbError> {
        crate::record_origins::postgres::resolve(&self.pool, dataset, record, item, kind).await
    }

    /// The creating origin of every imported aggregate of `kind` (ADR 0037 §4).
    pub(crate) async fn created_origins(
        &self,
        kind: &str,
    ) -> Result<Vec<(String, vitni_core::origin::RecordOrigin)>, DbError> {
        crate::record_origins::postgres::created(&self.pool, kind).await
    }

    /// The creating origin of one aggregate of `kind`, if an import created it (ADR 0037 §4).
    pub(crate) async fn created_origin(
        &self,
        kind: &str,
        aggregate_id: &str,
    ) -> Result<Option<vitni_core::origin::RecordOrigin>, DbError> {
        crate::record_origins::postgres::created_one(&self.pool, kind, aggregate_id).await
    }

    /// How many of `records` each dataset holds as an aggregate of `kind` (ADR 0037 §3).
    pub(crate) async fn origin_overlap(
        &self,
        kind: &str,
        records: &[String],
    ) -> Result<Vec<(vitni_core::origin::DatasetId, usize)>, DbError> {
        crate::record_origins::postgres::overlap(&self.pool, kind, records).await
    }

    /// The fingerprint the match keys were built under, if any (ADR 0038 §7).
    pub(crate) async fn match_keys_fingerprint(&self) -> Result<Option<String>, DbError> {
        crate::match_keys::postgres::fingerprint(&self.pool).await
    }

    /// Every matchable record touched since it was keyed.
    pub(crate) async fn match_dirty(&self) -> Result<Vec<crate::match_keys::DirtyRecord>, DbError> {
        crate::match_keys::postgres::dirty(&self.pool).await
    }

    /// Replaces the keys of `records` and clears `cleared`.
    pub(crate) async fn rekey_matches(
        &self,
        records: &[crate::match_keys::KeyedRecord],
        cleared: &[crate::match_keys::DirtyRecord],
    ) -> Result<(), DbError> {
        crate::match_keys::postgres::rekey(&self.pool, records, cleared).await
    }

    /// Replaces the whole match keys index.
    pub(crate) async fn reset_match_keys(
        &self,
        fingerprint: &str,
        records: &[crate::match_keys::KeyedRecord],
        cleared: &[crate::match_keys::DirtyRecord],
    ) -> Result<(), DbError> {
        crate::match_keys::postgres::reset(&self.pool, fingerprint, records, cleared).await
    }

    /// The ids of every record of `kind` holding a key `probe` meets.
    pub(crate) async fn match_candidates(
        &self,
        kind: vitni_core::matching::MatchableKind,
        probe: &vitni_core::matching::Probe,
    ) -> Result<Vec<String>, DbError> {
        crate::match_keys::postgres::candidates(&self.pool, kind, probe).await
    }

    /// Every `(aggregate_id, key)` of `kind`.
    pub(crate) async fn match_keys_of_kind(
        &self,
        kind: vitni_core::matching::MatchableKind,
    ) -> Result<Vec<(String, String)>, DbError> {
        crate::match_keys::postgres::keys_of_kind(&self.pool, kind).await
    }

    /// Every record origins row, as text columns in a stable order — for comparing a live index to
    /// a rebuilt one.
    pub(crate) async fn record_origins_dump(&self) -> Result<Vec<Vec<String>>, DbError> {
        crate::record_origins::postgres::all_rows(&self.pool).await
    }

    /// Every member of a `kind` cluster with its root (ADR 0039 §4).
    pub(crate) async fn identity_links(
        &self,
        kind: vitni_core::matching::MatchableKind,
    ) -> Result<Vec<crate::store::IdentityLink>, DbError> {
        crate::identity_links::postgres::links(&self.pool, kind).await
    }

    /// Every place a succession names `to`, from `place_id`'s perspective as a `from` endpoint
    /// (ADR 0026 §4), via the succession cross-reference index.
    pub(crate) async fn place_successors(
        &self,
        place_id: &str,
    ) -> Result<Vec<crate::store::PlaceSuccessionRecord>, DbError> {
        place_succession_index::postgres::successors(&self.pool, place_id).await
    }

    /// Every place a succession names `from`, from `place_id`'s perspective as a `to` endpoint
    /// (ADR 0026 §4), the symmetric counterpart of [`Self::place_successors`].
    pub(crate) async fn place_predecessors(
        &self,
        place_id: &str,
    ) -> Result<Vec<crate::store::PlaceSuccessionRecord>, DbError> {
        place_succession_index::postgres::predecessors(&self.pool, place_id).await
    }

    /// Every research note whose `subjects` set names the subject serialized under `subject_kind`
    /// (`Person`/`Family`/`Event`/`Place`) — the Postgres twin of
    /// [`crate::sqlite::SqliteStore::list_research_notes_for_subject`] (ADR 0028 §5).
    pub(crate) async fn list_research_notes_for_subject(
        &self,
        subject_kind: &str,
        subject_value: &str,
    ) -> Result<Vec<vitni_core::research_note::ResearchNoteView>, DbError> {
        postgres_query::list_views_by_subject(&self.pool, RESEARCH_NOTE_VIEW_TABLE, subject_kind, subject_value).await
    }
}

/// Brings every view table to the current shape (ADR 0032) — the Postgres twin of
/// [`crate::sqlite::migrate_sqlite_view_tables`]: drops any table left over from before the
/// generated `human_id` column existed (a generated column cannot be added by `ALTER TABLE`, so a
/// shape change is a drop, not a migration), recreates every table (idempotent when nothing was
/// stale), and indexes the human-id-bearing ones (`HUMAN_ID_VIEW_TABLES`). Returns which tables
/// were dropped, so the caller — which has the aggregate `State`/`View` types this untyped helper
/// does not — can replay their event logs to repopulate them.
async fn migrate_postgres_view_tables(pool: &Pool<Postgres>) -> Result<Vec<&'static str>, DbError> {
    let mut stale_tables = Vec::new();
    for &table in ALL_VIEW_TABLES {
        let was_stale = schema::postgres_view_table_is_stale(pool, table)
            .await
            .map_err(|e| DbError::Backend(format!("probing projection table {table}: {e}")))?;
        if was_stale {
            schema::drop_postgres_view_table(pool, table)
                .await
                .map_err(|e| DbError::Backend(format!("dropping stale projection table {table}: {e}")))?;
            stale_tables.push(table);
        }
        schema::create_postgres_view_table(pool, table)
            .await
            .map_err(|e| DbError::Backend(format!("creating projection table {table}: {e}")))?;
    }
    for &table in HUMAN_ID_VIEW_TABLES {
        schema::create_postgres_human_id_indexes(pool, table)
            .await
            .map_err(|e| DbError::Backend(format!("creating human_id indexes for {table}: {e}")))?;
    }
    Ok(stale_tables)
}

/// Replays aggregate `A`'s full event log through the record origins index (ADR 0037 §4), whose
/// rows are derived from the events themselves; `view_table` is `A`'s projection, which the index
/// reads `live` flags from.
async fn replay_record_origins<A>(
    pool: &Pool<Postgres>,
    view_table: &'static str,
    upcasters: Vec<Box<dyn EventUpcaster>>,
) -> Result<(), DbError>
where
    A: Aggregate,
    crate::record_origins::postgres::RecordOriginsQuery: cqrs_es::Query<A>,
{
    let query = crate::record_origins::postgres::RecordOriginsQuery::new(pool.clone(), view_table);
    QueryReplay::new(PostgresEventRepository::new(pool.clone()), query)
        .with_upcasters(upcasters)
        .replay_all()
        .await
        .map_err(|e| DbError::Backend(format!("rebuilding record origins from {view_table}: {e}")))
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
