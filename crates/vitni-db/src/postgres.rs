//! Postgres backend for [`Store`](crate::store::Store) — private wiring (ADR 0002).
//!
//! The server twin of [`sqlite`](crate::sqlite): one `cqrs-es` framework per aggregate over the
//! `postgres-es` repositories, sharing the read-model pool. The per-aggregate fields, wiring,
//! methods, and rebuild are generated from the [`registry`](crate::registry), identically to the
//! SQLite backend. Everything here (`sqlx`, `postgres-es`, `cqrs-es`) is an implementation detail;
//! only [`crate::store`] re-exposes it, in engine-neutral terms.

use std::time::Duration;

use cqrs_es::persist::PersistedEventStore;
use cqrs_es::{Aggregate, AggregateContext as _, CqrsFramework, EventStore as _, Query, View};
use postgres_es::{PostgresEventRepository, postgres_cqrs};
use sqlx::postgres::PgPoolOptions;
use sqlx::{Pool, Postgres};

use crate::place_succession_index;
use crate::postgres_query;
use crate::registry::{
    for_each_db_aggregate, for_each_db_by_ids_aggregate, for_each_db_external_id_aggregate,
    for_each_db_human_id_aggregate, for_each_db_listed_aggregate,
};
use crate::resolver::PostgresRefStore;
use crate::schema;
use crate::store::{CommandError, DbError, ListPage, map_aggregate_error};
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

/// Builds one aggregate's `CqrsFramework` in `open()` over its `$queries`, matching the registry
/// `wiring` column: a plain unit `Services`, a projection-reading resolver (the §9 aggregate tax), or
/// the hand-assembled Event store that carries upcasters at load (ADR 0010).
macro_rules! postgres_open_cqrs {
    ($pool:ident, $queries:expr, (plain)) => {
        postgres_cqrs($pool.clone(), $queries, ())
    };
    ($pool:ident, $queries:expr, (resolver $resolver:path)) => {
        postgres_cqrs(
            $pool.clone(),
            $queries,
            <$resolver>::new(PostgresRefStore::shared($pool.clone())),
        )
    };
    ($pool:ident, $queries:expr, (event $resolver:path)) => {{
        let store = PersistedEventStore::new_event_store(PostgresEventRepository::new($pool.clone()))
            .with_upcasters(vitni_core::event::upcasters());
        CqrsFramework::new(
            store,
            $queries,
            <$resolver>::new(PostgresRefStore::shared($pool.clone())),
        )
    }};
}

/// Every query one aggregate's commits dispatch to, in order — the twin of the SQLite backend's
/// `sqlite_queries!`.
macro_rules! postgres_queries {
    ($snake:ident, $pool:expr, $State:ty, $View:ty, $table_const:ident, $upcasters:expr) => {{
        let mut queries: Vec<Box<dyn Query<$State>>> =
            vec![Box::new(projection_query::<$View, $State>($pool, $table_const, || {
                $upcasters
            }))];
        postgres_side_indexes!($snake, $pool, queries);
        postgres_list_table!($snake, $pool, queries);
        queries.push(Box::new(crate::record_origins::postgres::RecordOriginsQuery::new(
            $pool.clone(),
            $table_const,
        )));
        queries.push(Box::new(crate::match_keys::postgres::MatchDirtyQuery::new(
            $pool.clone(),
        )));
        queries
    }};
}

/// Pushes the list table query (ADR 0052 §5) onto the queries of every listed aggregate — after its
/// side indexes, so the record links its dependents are found through are current. `import_run` is not
/// listed.
macro_rules! postgres_list_table {
    (person, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::list_table::postgres::ListTableQuery::<
            vitni_core::person::PersonView,
        >::new($pool.clone())));
    };
    (family, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::list_table::postgres::ListTableQuery::<
            vitni_core::family::FamilyView,
        >::new($pool.clone())));
    };
    (place, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::list_table::postgres::ListTableQuery::<
            vitni_core::place::PlaceView,
        >::new($pool.clone())));
    };
    (source, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::list_table::postgres::ListTableQuery::<
            vitni_core::source::SourceView,
        >::new($pool.clone())));
    };
    (citation, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::list_table::postgres::ListTableQuery::<
            vitni_core::citation::CitationView,
        >::new($pool.clone())));
    };
    (event, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::list_table::postgres::ListTableQuery::<
            vitni_core::event::EventView,
        >::new($pool.clone())));
    };
    (dna_test, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::list_table::postgres::ListTableQuery::<
            vitni_core::dna_test::DnaTestView,
        >::new($pool.clone())));
    };
    (dna_match, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::list_table::postgres::ListTableQuery::<
            vitni_core::dna_match::DnaMatchView,
        >::new($pool.clone())));
    };
    (repository, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::list_table::postgres::ListTableQuery::<
            vitni_core::repository::RepositoryView,
        >::new($pool.clone())));
    };
    (note, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::list_table::postgres::ListTableQuery::<
            vitni_core::note::NoteView,
        >::new($pool.clone())));
    };
    (media, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::list_table::postgres::ListTableQuery::<
            vitni_core::media::MediaView,
        >::new($pool.clone())));
    };
    (research_note, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::list_table::postgres::ListTableQuery::<
            vitni_core::research_note::ResearchNoteView,
        >::new($pool.clone())));
    };
    (tag, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::list_table::postgres::ListTableQuery::<
            vitni_core::tag::TagView,
        >::new($pool.clone())));
    };
    ($other:ident, $pool:expr, $queries:ident) => {};
}

/// Pushes the derived side indexes onto the queries of the one aggregate each is fed by: the identity
/// cluster index (ADR 0039 §4) for every matchable kind but `tag`, the record links index (ADR 0047)
/// for the linking kinds, and the geometry (ADR 0024 §3) and succession (ADR 0026 §4) indexes for
/// `place`. Dispatches on the registry's literal `$snake` token — the same "wiring by tag" shape as
/// [`postgres_open_cqrs!`].
macro_rules! postgres_side_indexes {
    (person, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::person::PersonView,
        >::new($pool.clone())));
        $queries.push(Box::new(crate::record_links::postgres::RecordLinksQuery::<
            vitni_core::person::PersonView,
        >::new($pool.clone())));
    };
    (event, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::event::EventView,
        >::new($pool.clone())));
        $queries.push(Box::new(crate::record_links::postgres::RecordLinksQuery::<
            vitni_core::event::EventView,
        >::new($pool.clone())));
    };
    (family, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::family::FamilyView,
        >::new($pool.clone())));
        $queries.push(Box::new(crate::record_links::postgres::RecordLinksQuery::<
            vitni_core::family::FamilyView,
        >::new($pool.clone())));
    };
    (place, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::geo_index::postgres::PlaceGeometryIndexQuery::new(
            $pool.clone(),
        )));
        $queries.push(Box::new(
            place_succession_index::postgres::PlaceSuccessionIndexQuery::new($pool.clone()),
        ));
        $queries.push(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::place::PlaceView,
        >::new($pool.clone())));
        $queries.push(Box::new(crate::record_links::postgres::RecordLinksQuery::<
            vitni_core::place::PlaceView,
        >::new($pool.clone())));
    };
    (source, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::source::SourceView,
        >::new($pool.clone())));
        $queries.push(Box::new(crate::record_links::postgres::RecordLinksQuery::<
            vitni_core::source::SourceView,
        >::new($pool.clone())));
    };
    (citation, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::citation::CitationView,
        >::new($pool.clone())));
        $queries.push(Box::new(crate::record_links::postgres::RecordLinksQuery::<
            vitni_core::citation::CitationView,
        >::new($pool.clone())));
    };
    (repository, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::repository::RepositoryView,
        >::new($pool.clone())));
        $queries.push(Box::new(crate::record_links::postgres::RecordLinksQuery::<
            vitni_core::repository::RepositoryView,
        >::new($pool.clone())));
    };
    (note, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::note::NoteView,
        >::new($pool.clone())));
        $queries.push(Box::new(crate::record_links::postgres::RecordLinksQuery::<
            vitni_core::note::NoteView,
        >::new($pool.clone())));
    };
    (media, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::identity_links::postgres::IdentityLinksQuery::<
            vitni_core::media::MediaView,
        >::new($pool.clone())));
        $queries.push(Box::new(crate::record_links::postgres::RecordLinksQuery::<
            vitni_core::media::MediaView,
        >::new($pool.clone())));
    };
    (dna_test, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::record_links::postgres::RecordLinksQuery::<
            vitni_core::dna_test::DnaTestView,
        >::new($pool.clone())));
    };
    (dna_match, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::record_links::postgres::RecordLinksQuery::<
            vitni_core::dna_match::DnaMatchView,
        >::new($pool.clone())));
    };
    (research_note, $pool:expr, $queries:ident) => {
        $queries.push(Box::new(crate::record_links::postgres::RecordLinksQuery::<
            vitni_core::research_note::ResearchNoteView,
        >::new($pool.clone())));
    };
    ($other:ident, $pool:expr, $queries:ident) => {};
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
            /// The unreadable events the open found, each in a journalled aggregate (ADR 0051).
            unreadable: Vec<crate::raw::UnreadableEvent>,
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
                // Drops and recreates any view table left over from before the `human_id` (ADR 0032)
                // or `sequence` (ADR 0050) column existed, creates every table fresh, and indexes the
                // human-id-bearing ones; returns which tables were dropped, so their aggregate's event log
                // can be replayed to repopulate them below (rebuilding needs the aggregate's
                // `State`/`View` types, which this untyped helper does not have).
                let stale_tables = migrate_postgres_view_tables(&pool).await?;
                // The Place geometry spatial index (ADR 0024 §3) is derived from the Place projection and
                // keyed by its own table; its `Query` is appended only to the Place framework below, and a
                // new index is filled once the projections are open.
                let geometry_is_new = crate::geo_index::postgres::create_tables(&pool)
                    .await
                    .map_err(|e| DbError::Backend(format!("creating place geometry index: {e}")))?;
                // The Place succession cross-reference index (ADR 0026 §4) is not one of the generic
                // per-aggregate projections: it is derived from the Place projection and keyed by its
                // own tables. Its DDL is created up front, and its `Query` is appended only to the
                // Place framework below (`postgres_wire_side_indexes!`).
                place_succession_index::postgres::create_tables(&pool)
                    .await
                    .map_err(|e| DbError::Backend(format!("creating place succession index: {e}")))?;
                // The match keys blocking index (ADR 0038 §7): every commit marks the matchable
                // aggregate it touched, and the app layer rekeys it before the next lookup.
                crate::match_keys::postgres::create_tables(&pool)
                    .await
                    .map_err(|e| DbError::Backend(format!("creating match keys index: {e}")))?;
                // The match pairs projection (ADR 0048), marked dirty by the same query.
                crate::match_pairs::postgres::create_tables(&pool)
                    .await
                    .map_err(|e| DbError::Backend(format!("creating match pairs: {e}")))?;
                // The identity cluster index (ADR 0039 §4) is derived from the projections of every
                // matchable kind; its `Query` is appended to those frameworks below, and a workspace that
                // predates it gets it filled once the projections are open.
                let identity_is_new = crate::identity_links::postgres::create_tables(&pool)
                    .await
                    .map_err(|e| DbError::Backend(format!("creating identity index: {e}")))?;
                // The record origins index (ADR 0037 §4) is fed by every aggregate. One that is new, or
                // whose last fill or rebuild never finished, is refilled from the log below.
                let origins_need_backfill = crate::record_origins::postgres::create_tables(&pool)
                    .await
                    .map_err(|e| DbError::Backend(format!("creating record origins index: {e}")))?;
                $(
                    let $snake = postgres_open_cqrs!(pool, postgres_queries!($snake, &pool, $State, $View, $table_const, $upcasters), $wiring);
                    if stale_tables.contains(&$table_const) {
                        tracing::info!(
                            table = $table_const,
                            "projection table predated the human_id or sequence column; rebuilding from the event log"
                        );
                        // An unreadable event it meets is journalled, and reported by the catch-up below.
                        let _journalled = rebuild_view::<$State, $View>(&pool, $table_const, || $upcasters).await?;
                    }
                )+
                if origins_need_backfill {
                    Self::backfill_record_origins(&pool).await?;
                }
                let unreadable = Self::catch_up_journal(&pool).await?;
                if geometry_is_new {
                    crate::geo_index::postgres::rebuild_index(&pool).await?;
                }
                if identity_is_new {
                    crate::identity_links::postgres::rebuild_index(&pool).await?;
                }
                // The record links index (ADR 0047) mirrors the references the person, family, event and
                // citation projections hold; its `Query` is appended to those frameworks above, and a
                // workspace that predates it gets it created and filled from the open projections.
                crate::record_links::postgres::create_filled(&pool).await?;
                // The list tables (ADR 0052 §5) are derived from the projections and the record links; a
                // workspace that predates one gets it created and filled, its tag links with it.
                crate::list_table::postgres::create_filled(&pool).await?;
                Ok(Self { $($snake,)+ pool, unreadable })
            }

            $(
                pub(crate) async fn $execute(
                    &self,
                    aggregate_id: &str,
                    command: $Cmd,
                ) -> Result<(), CommandError<$Err>> {
                    // Journalled until every query has run: the events commit first, so a process
                    // killed in between leaves the entry for the next open to replay (ADR 0050).
                    let entry = crate::record_origins::postgres::begin_write(&self.pool, <$State as Aggregate>::TYPE, aggregate_id)
                        .await
                        .map_err(CommandError::Store)?;
                    let (result, failures) = crate::projection_failures::watch(self.$snake.execute(aggregate_id, command)).await;
                    if let (Ok(()), Some(failures)) = (&result, failures) {
                        // Left journalled, so the next open replays the aggregate into what failed.
                        return Err(CommandError::Store(failures.into_error(
                            "the events were committed, but their projection failed to update (reopening the workspace repairs it)",
                        )));
                    }
                    if let Err(error) = crate::record_origins::postgres::end_write(&self.pool, entry).await {
                        tracing::warn!(%error, aggregate_id, "a write stays journalled; the next open replays it again");
                    }
                    result.map_err(map_aggregate_error)
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

            /// The unreadable events the open found (ADR 0051).
            pub(crate) fn unreadable_events(&self) -> &[crate::raw::UnreadableEvent] {
                &self.unreadable
            }

            /// Rebuilds every projection from the event log (ADR 0010): each view table is cleared,
            /// then its aggregate's full history is replayed back into it through the same
            /// projection query the live store uses (ADR 0050), with the Event aggregate's upcasters
            /// applied. Returns the unreadable events it met, journalled (ADR 0051). A maintenance
            /// operation — the caller must ensure no commands run concurrently.
            pub(crate) async fn rebuild_projections(&self) -> Result<Vec<crate::raw::UnreadableEvent>, DbError> {
                Self::rebuild_projections_on(&self.pool).await
            }

            /// [`Self::rebuild_projections`] over `pool`, which [`Self::replace_all_events`] points at
            /// its exclusive transaction.
            async fn rebuild_projections_on(pool: &Pool<Postgres>) -> Result<Vec<crate::raw::UnreadableEvent>, DbError> {
                let mut unreadable = Vec::new();
                $(
                    unreadable.extend(rebuild_view::<$State, $View>(pool, $table_const, || $upcasters).await?);
                )+
                // Place's succession cross-reference index (ADR 0026 §4) is derived from the (now
                // freshly rebuilt) Place projection above, not replayed from raw events itself; so is the
                // geometry index (ADR 0024 §3).
                crate::geo_index::postgres::rebuild_index(pool).await?;
                place_succession_index::postgres::rebuild_index(pool).await?;
                // The identity clusters (ADR 0039 §4) are derived from the rebuilt projections of every
                // matchable kind.
                crate::identity_links::postgres::rebuild_index(pool).await?;
                // The record links (ADR 0047) are derived from the rebuilt person, family, event and
                // citation projections.
                crate::record_links::postgres::rebuild_index(pool).await?;
                // The list tables read the rebuilt projections.
                crate::list_table::postgres::rebuild(pool).await?;
                // The match keys need the name-culture packs, which only the app layer has: forgetting
                // what the index was built under makes it rebuild on next use.
                crate::match_keys::postgres::clear_state(pool).await?;
                crate::match_pairs::postgres::clear_state(pool).await?;
                // The record origins index is replayed from the raw events, after the projections
                // its `live` flags are read from. Unmarked first, so a rebuild cut short is redone by
                // the next open.
                crate::record_origins::postgres::mark_incomplete(pool).await?;
                unreadable.extend(Self::refill_record_origins_on(pool).await?);
                unreadable.sort();
                unreadable.dedup();
                Ok(unreadable)
            }

            /// Fills the record origins index from the log in one transaction, so an open cut short
            /// leaves it as it was, still unmarked; a second open waiting on the lock finds it filled.
            async fn backfill_record_origins(pool: &Pool<Postgres>) -> Result<(), DbError> {
                let exclusive = crate::exclusive::ExclusivePool::begin(pool).await?;
                let outcome = async {
                    if crate::record_origins::postgres::lock_for_refill(exclusive.pool()).await? {
                        return Ok(());
                    }
                    // An unreadable event it meets is journalled, and reported by the catch-up after it.
                    Self::refill_record_origins_on(exclusive.pool()).await.map(|_journalled| ())
                }
                .await;
                exclusive.finish(outcome).await
            }

            /// Replays every aggregate a command left journalled through all of its queries, as one
            /// batch — one whose process was killed after its events committed and before every query
            /// ran, or whose projection failed to update (ADR 0050). Each query skips what it already
            /// holds: the projection folds only the events after its last, the indexes are keyed by
            /// event or re-derived from the projection. An aggregate that cannot be replayed is logged
            /// and stays journalled, so the workspace still opens and a rebuild can repair it.
            ///
            /// An aggregate holding an unreadable event is folded up to it and stays journalled too,
            /// so every open reports it until a version that reads it folds the rest (ADR 0051).
            /// Returns those events. The entry records the event its prefix was replayed up to, so a
            /// later open that finds the same one dispatches nothing.
            async fn catch_up_journal(pool: &Pool<Postgres>) -> Result<Vec<crate::raw::UnreadableEvent>, DbError> {
                let mut unreadable = Vec::new();
                for write in crate::record_origins::postgres::pending_writes(pool).await? {
                    let replayed_to = write.replayed_to.and_then(|sequence| usize::try_from(sequence).ok());
                    match Self::replay_journalled(pool, &write.aggregate_type, &write.aggregate_id, replayed_to).await {
                        Ok(None) => crate::record_origins::postgres::end_write(pool, write.entry).await?,
                        Ok(Some(event)) => {
                            tracing::warn!(%event, "a journalled aggregate holds an unreadable event; it stays journalled");
                            if replayed_to != Some(event.sequence) {
                                let sequence = i64::try_from(event.sequence)
                                    .map_err(|_| DbError::Backend(format!("sequence {} does not fit the journal", event.sequence)))?;
                                crate::record_origins::postgres::mark_replayed(pool, write.entry, sequence).await?;
                            }
                            unreadable.push(event);
                        }
                        Err(error) => tracing::error!(
                            aggregate_type = write.aggregate_type,
                            aggregate_id = write.aggregate_id,
                            %error,
                            "a journalled aggregate could not be replayed; it stays journalled"
                        ),
                    }
                }
                unreadable.sort();
                unreadable.dedup();
                Ok(unreadable)
            }

            /// Replays one journalled aggregate's log through its queries — unless `replayed_to` says an
            /// earlier open already did, up to the same unreadable event — returning the unreadable
            /// event that stops it, if any.
            async fn replay_journalled(
                pool: &Pool<Postgres>,
                aggregate_type: &str,
                aggregate_id: &str,
                replayed_to: Option<usize>,
            ) -> Result<Option<crate::raw::UnreadableEvent>, DbError> {
                $(
                    if aggregate_type == <$State as Aggregate>::TYPE {
                        let log = crate::projection::EventLog::new(PostgresEventRepository::new(pool.clone()), || $upcasters);
                        let queries = crate::projection::QueryChain(postgres_queries!($snake, pool, $State, $View, $table_const, $upcasters));
                        let context = format!("replaying {aggregate_type} {aggregate_id}");
                        return crate::projection::catch_up(&log, &queries, aggregate_id, replayed_to, &context).await;
                    }
                )+
                Err(DbError::Backend(format!("the write journal names an unknown aggregate type {aggregate_type}")))
            }

            /// Clears the record origins index, replays every aggregate's log into it, and marks it
            /// complete. Returns the unreadable events it met, journalled (ADR 0051).
            async fn refill_record_origins_on(pool: &Pool<Postgres>) -> Result<Vec<crate::raw::UnreadableEvent>, DbError> {
                crate::record_origins::postgres::clear_table(pool).await?;
                let mut unreadable = Vec::new();
                $(
                    unreadable.extend(replay_record_origins::<$State>(pool, $table_const, || $upcasters).await?);
                )+
                crate::record_origins::postgres::mark_complete(pool).await?;
                Ok(unreadable)
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

macro_rules! postgres_by_ids_methods {
    ($(($snake:ident, $by_ids:ident, $table_const:ident, $View:ty)),+ $(,)?) => {
        impl PostgresStore {
            $(
                pub(crate) async fn $by_ids(&self, ids: &[String]) -> Result<Vec<$View>, DbError> {
                    postgres_query::views_by_ids(&self.pool, $table_const, ids).await
                }
            )+
        }
    };
}

for_each_db_by_ids_aggregate!(postgres_by_ids_methods);

macro_rules! postgres_list_page_methods {
    ($(($snake:ident, $list_page:ident, $table_const:ident, $View:ty, $kind:expr)),+ $(,)?) => {
        impl PostgresStore {
            $(
                pub(crate) async fn $list_page(&self, page: ListPage) -> Result<Vec<$View>, DbError> {
                    let kind: Option<vitni_core::matching::MatchableKind> = $kind;
                    postgres_query::list_views_page(&self.pool, $table_const, kind.map(|kind| kind.as_str()), page).await
                }
            )+
        }
    };
}

for_each_db_listed_aggregate!(postgres_list_page_methods);

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

    /// Swaps the log for `rows` and rebuilds every projection from them, all in one transaction
    /// (ADR 0044 §2): on any failure the store is left exactly as it was.
    pub(crate) async fn replace_all_events(
        &self,
        rows: impl IntoIterator<Item = Result<crate::raw::RawEvent, DbError>>,
    ) -> Result<u64, DbError> {
        let exclusive = crate::exclusive::ExclusivePool::begin(&self.pool).await?;
        let outcome = async {
            postgres_query::discard_all_events(exclusive.pool()).await?;
            let inserted = postgres_query::insert_raw_events(exclusive.pool(), rows).await?;
            // A restore decodes every row before it gets here (ADR 0041), so no event is unreadable.
            Self::rebuild_projections_on(exclusive.pool()).await?;
            Ok(inserted)
        }
        .await;
        exclusive.finish(outcome).await
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

    /// The creating origin of each imported aggregate of `kind` among `ids` (ADR 0037 §4).
    pub(crate) async fn created_origins_of(
        &self,
        kind: &str,
        ids: &[String],
    ) -> Result<Vec<(String, vitni_core::origin::RecordOrigin)>, DbError> {
        crate::record_origins::postgres::created_among(&self.pool, kind, ids).await
    }

    /// Every `(source, target)` of `relation` whose target is one of `targets` (ADR 0047).
    pub(crate) async fn linking(
        &self,
        relation: crate::record_links::RecordLink,
        targets: &[String],
    ) -> Result<Vec<(String, String)>, DbError> {
        crate::record_links::postgres::linking(&self.pool, relation, targets).await
    }

    /// Evaluates a list query over its kind's list table (ADR 0052 §4).
    pub(crate) async fn list_query(
        &self,
        spec: &crate::list_table::ListSpec,
    ) -> Result<crate::list_table::ListResult, DbError> {
        crate::list_table::postgres::query(&self.pool, spec).await
    }

    /// The records carrying `tag`, as cluster roots (ADR 0052 §6).
    pub(crate) async fn tag_uses(&self, tag: &str) -> Result<Vec<(crate::list_table::ListedKind, String)>, DbError> {
        crate::list_table::postgres::tag_uses(&self.pool, tag).await
    }

    /// How many records carry each tag in use.
    pub(crate) async fn tag_use_counts(&self) -> Result<Vec<(String, u64)>, DbError> {
        crate::list_table::postgres::tag_use_counts(&self.pool).await
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

    /// The origin of every live assertion on one aggregate of `kind` (ADR 0037 §2).
    pub(crate) async fn assertion_origins(
        &self,
        kind: &str,
        aggregate_id: &str,
    ) -> Result<Vec<(vitni_core::ids::AssertionId, vitni_core::origin::RecordOrigin)>, DbError> {
        crate::record_origins::postgres::assertion_origins(&self.pool, kind, aggregate_id).await
    }

    /// The aggregates one import run wrote to, grouped by source record, the last first.
    pub(crate) async fn run_groups(
        &self,
        run: vitni_core::ids::ImportRunId,
        offset: u32,
        limit: u32,
    ) -> Result<Vec<crate::record_origins::RunGroupRow>, DbError> {
        crate::record_origins::postgres::run_groups(&self.pool, run, offset, limit).await
    }

    /// How many aggregates of each kind one import run wrote to, events per type.
    pub(crate) async fn run_kind_counts(
        &self,
        run: vitni_core::ids::ImportRunId,
    ) -> Result<Vec<crate::record_origins::RunKindCount>, DbError> {
        crate::record_origins::postgres::run_kind_counts(&self.pool, run).await
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
        crate::match_keys::postgres::dirty(crate::match_keys::MATCH_DIRTY_TABLE, &self.pool).await
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
    /// The fingerprint the match pairs were scored under, if any (ADR 0048).
    pub(crate) async fn match_pairs_fingerprint(&self) -> Result<Option<String>, DbError> {
        crate::match_pairs::postgres::fingerprint(&self.pool).await
    }

    /// Every matchable record touched since its pairs were refreshed.
    pub(crate) async fn match_pairs_dirty(&self) -> Result<Vec<crate::match_keys::DirtyRecord>, DbError> {
        crate::match_pairs::postgres::dirty(&self.pool).await
    }

    /// Replaces the pairs each refresh covers, and clears `cleared` at the generations read.
    pub(crate) async fn refresh_match_pairs(
        &self,
        refreshes: &[crate::match_pairs::PairRefresh],
        cleared: &[crate::match_keys::DirtyRecord],
    ) -> Result<(), DbError> {
        crate::match_pairs::postgres::refresh(&self.pool, refreshes, cleared).await
    }

    /// Replaces every match pair, unless another rebuild already scored them under `fingerprint`.
    pub(crate) async fn reset_match_pairs(
        &self,
        fingerprint: &str,
        pairs: &[crate::match_pairs::MatchPair],
        cleared: &[crate::match_keys::DirtyRecord],
    ) -> Result<(), DbError> {
        crate::match_pairs::postgres::reset(&self.pool, fingerprint, pairs, cleared).await
    }

    /// The undecided match pairs from `min_band` up, strongest first.
    pub(crate) async fn match_pairs(
        &self,
        kinds: &[vitni_core::matching::MatchableKind],
        min_band: vitni_core::matching::MatchBand,
        limit: Option<usize>,
    ) -> Result<Vec<crate::match_pairs::MatchPair>, DbError> {
        crate::match_pairs::postgres::pairs(&self.pool, kinds, min_band, limit).await
    }

    /// How many undecided match pairs from `min_band` up each kind holds.
    pub(crate) async fn match_pair_counts(
        &self,
        min_band: vitni_core::matching::MatchBand,
    ) -> Result<Vec<(vitni_core::matching::MatchableKind, usize)>, DbError> {
        crate::match_pairs::postgres::counts(&self.pool, min_band).await
    }

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

    /// Every live `kind` distinction as `(record, other)` (ADR 0039 §4).
    pub(crate) async fn identity_distinctions(
        &self,
        kind: vitni_core::matching::MatchableKind,
    ) -> Result<Vec<(String, String)>, DbError> {
        crate::identity_links::postgres::distinctions(&self.pool, kind).await
    }

    /// Every member of a `kind` cluster with its root (ADR 0039 §4).
    pub(crate) async fn identity_links(
        &self,
        kind: vitni_core::matching::MatchableKind,
    ) -> Result<Vec<crate::store::IdentityLink>, DbError> {
        crate::identity_links::postgres::links(&self.pool, kind).await
    }

    /// Every member of each `kind` cluster that holds one of `ids`, with its root (ADR 0039 §4).
    pub(crate) async fn identity_links_of(
        &self,
        kind: vitni_core::matching::MatchableKind,
        ids: &[String],
    ) -> Result<Vec<crate::store::IdentityLink>, DbError> {
        crate::identity_links::postgres::links_of(&self.pool, kind, ids).await
    }

    /// Every place with a geometry overlapping the given bounding box (ADR 0024 §3), via the GiST-backed
    /// geometry index.
    pub(crate) async fn places_in_bbox(
        &self,
        min_lat: f64,
        min_lon: f64,
        max_lat: f64,
        max_lon: f64,
    ) -> Result<Vec<String>, DbError> {
        crate::geo_index::postgres::places_in_bbox(&self.pool, min_lat, min_lon, max_lat, max_lon).await
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
/// reads `live` flags from. Returns the unreadable events it met, journalled (ADR 0051).
async fn replay_record_origins<A>(
    pool: &Pool<Postgres>,
    view_table: &'static str,
    upcasters: crate::projection::Upcasters,
) -> Result<Vec<crate::raw::UnreadableEvent>, DbError>
where
    A: Aggregate,
    crate::record_origins::postgres::RecordOriginsQuery: cqrs_es::Query<A>,
{
    let query = crate::record_origins::postgres::RecordOriginsQuery::new(pool.clone(), view_table);
    replay_all::<A>(
        pool,
        &query,
        upcasters,
        &format!("rebuilding record origins from {view_table}"),
    )
    .await
}

/// Clears one view table and replays its aggregate's full event log back into it (ADR 0010, 0050).
/// Returns the unreadable events it met, journalled (ADR 0051).
///
/// `upcasters` migrate historical payloads during the replay; pass an empty vec for aggregates
/// whose schema has not evolved.
async fn rebuild_view<A, V>(
    pool: &Pool<Postgres>,
    table: &'static str,
    upcasters: crate::projection::Upcasters,
) -> Result<Vec<crate::raw::UnreadableEvent>, DbError>
where
    A: Aggregate,
    V: View<A>,
{
    schema::clear_postgres_view_table(pool, table)
        .await
        .map_err(|e| DbError::Backend(format!("clearing projection {table}: {e}")))?;
    let query = projection_query::<V, A>(pool, table, upcasters);
    replay_all::<A>(pool, &query, upcasters, &format!("rebuilding projection {table}")).await
}

/// Replays every aggregate `A` instance's log through `query`, journalling each one holding an
/// unreadable event so every open replays it until it reads (ADR 0051), and returns those events.
async fn replay_all<A: Aggregate>(
    pool: &Pool<Postgres>,
    query: &impl Query<A>,
    upcasters: crate::projection::Upcasters,
    context: &str,
) -> Result<Vec<crate::raw::UnreadableEvent>, DbError> {
    let aggregate_ids = postgres_query::aggregate_ids(pool, A::TYPE).await?;
    let log = crate::projection::EventLog::new(PostgresEventRepository::new(pool.clone()), upcasters);
    let unreadable = crate::projection::replay(&log, query, &aggregate_ids, context).await?;
    for event in &unreadable {
        crate::record_origins::postgres::journal_once(pool, &event.aggregate_type, &event.aggregate_id).await?;
    }
    Ok(unreadable)
}

/// The query writing aggregate `A`'s projection into `table` (ADR 0050), its failures reported
/// through [`crate::projection_failures`] rather than dropped.
fn projection_query<V, A>(
    pool: &Pool<Postgres>,
    table: &'static str,
    upcasters: crate::projection::Upcasters,
) -> crate::projection::ProjectionQuery<V, A, Pool<Postgres>, PostgresEventRepository>
where
    A: Aggregate,
    V: View<A>,
{
    let log = crate::projection::EventLog::new(PostgresEventRepository::new(pool.clone()), upcasters);
    crate::projection::ProjectionQuery::new(pool.clone(), log, table)
}
