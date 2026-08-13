//! Initial table creation for a workspace database (ADR 0002: `vitni-db` owns the schema).
//!
//! `cqrs-es` backends expect an `events` table (and a `snapshots` table) with a fixed column
//! layout; the SQLite backend ships no migrations, so we own the DDL here. The column shape
//! mirrors what `sqlite-es` reads and writes: `(aggregate_type, aggregate_id, sequence,
//! event_type, event_version, payload, metadata)` keyed by `(aggregate_type, aggregate_id,
//! sequence)` — the `UNIQUE` key that *is* the optimistic-concurrency guard.
//!
//! Every view table also carries a fourth, `GENERATED ALWAYS ... STORED` `human_id` column
//! (ADR 0032) mirroring `payload->state->human_id`, plus two indexes over it for the human-id
//! aggregates (`HUMAN_ID_VIEW_TABLES`). SQLite cannot `ALTER TABLE ADD COLUMN ... STORED`, so a
//! view table created before this column existed is dropped and rebuilt on open, not migrated in
//! place — [`sqlite_view_table_is_stale`]/[`postgres_view_table_is_stale`] detect that case.

#[cfg(feature = "postgres")]
use sqlx::Postgres;
#[cfg(feature = "sqlite")]
use sqlx::Sqlite;
use sqlx::{Pool, Row};

/// The event log: one row per committed event, ordered per stream by `sequence`.
#[cfg(feature = "sqlite")]
const CREATE_EVENTS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS events (
    aggregate_type TEXT    NOT NULL,
    aggregate_id   TEXT    NOT NULL,
    sequence       INTEGER NOT NULL CHECK (sequence >= 0),
    event_type     TEXT    NOT NULL,
    event_version  TEXT    NOT NULL,
    payload        TEXT    NOT NULL,
    metadata       TEXT    NOT NULL,
    PRIMARY KEY (aggregate_type, aggregate_id, sequence)
)";

/// The optional snapshot store (unused by the event-sourced store, created for completeness).
#[cfg(feature = "sqlite")]
const CREATE_SNAPSHOTS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS snapshots (
    aggregate_type   TEXT    NOT NULL,
    aggregate_id     TEXT    NOT NULL,
    last_sequence    INTEGER NOT NULL CHECK (last_sequence >= 0),
    current_snapshot INTEGER NOT NULL CHECK (current_snapshot >= 0),
    payload          TEXT    NOT NULL,
    PRIMARY KEY (aggregate_type, aggregate_id)
)";

/// Creates the core event-store tables on a fresh SQLite workspace database.
///
/// Idempotent (`IF NOT EXISTS`), so it is safe to call on every open. Projection/view tables are
/// created per view with [`create_sqlite_view_table`].
///
/// # Errors
///
/// Returns the `sqlx` error if a `CREATE TABLE` statement fails.
#[cfg(feature = "sqlite")]
pub async fn init_sqlite(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(CREATE_EVENTS_TABLE).execute(pool).await?;
    sqlx::query(CREATE_SNAPSHOTS_TABLE).execute(pool).await?;
    Ok(())
}

/// Creates a `cqrs-es` view (read-model) table with the layout `SqliteViewRepository` expects:
/// `(view_id PRIMARY KEY, version, payload)`, plus the generated `human_id` column (ADR 0032)
/// every view table carries; `sqlite-es` writes explicit column lists, so the extra column is
/// invisible to it.
///
/// `view_name` must be a trusted, code-supplied identifier (it is interpolated into the DDL).
///
/// # Errors
///
/// Returns the `sqlx` error if the `CREATE TABLE` statement fails.
#[cfg(feature = "sqlite")]
pub async fn create_sqlite_view_table(pool: &Pool<Sqlite>, view_name: &str) -> Result<(), sqlx::Error> {
    let ddl = format!(
        "CREATE TABLE IF NOT EXISTS {view_name} (
            view_id  TEXT    NOT NULL PRIMARY KEY,
            version  INTEGER NOT NULL,
            payload  TEXT    NOT NULL,
            human_id TEXT GENERATED ALWAYS AS (json_extract(payload, '$.state.human_id')) STORED
        )"
    );
    sqlx::query(&ddl).execute(pool).await?;
    Ok(())
}

/// Returns whether `view_name` exists in SQLite but predates the generated `human_id` column
/// (ADR 0032) — the signal that it must be dropped and rebuilt rather than left in place. A
/// nonexistent table (nothing to migrate; the caller creates it fresh) is not stale.
///
/// Uses `PRAGMA table_xinfo`, not `table_info`: SQLite reports a `GENERATED ALWAYS ... STORED`
/// column as hidden, and `table_info` omits hidden columns (`table_xinfo` is the variant that
/// includes them) — `table_info` would see every table, migrated or not, as missing `human_id`.
///
/// # Errors
///
/// Returns the `sqlx` error if the `PRAGMA table_xinfo` query fails.
#[cfg(feature = "sqlite")]
pub async fn sqlite_view_table_is_stale(pool: &Pool<Sqlite>, view_name: &str) -> Result<bool, sqlx::Error> {
    let rows = sqlx::query(&format!("PRAGMA table_xinfo({view_name})"))
        .fetch_all(pool)
        .await?;
    if rows.is_empty() {
        return Ok(false);
    }
    let has_human_id = rows.iter().any(|row| row.get::<String, _>("name") == "human_id");
    Ok(!has_human_id)
}

/// Drops a SQLite view (read-model) table outright — the migration path for a shape change SQLite
/// cannot `ALTER TABLE` its way into (ADR 0032): the caller recreates it in the new shape and
/// replays the event log to repopulate it.
///
/// `view_name` must be a trusted, code-supplied identifier (it is interpolated into the DDL).
///
/// # Errors
///
/// Returns the `sqlx` error if the `DROP TABLE` statement fails.
#[cfg(feature = "sqlite")]
pub async fn drop_sqlite_view_table(pool: &Pool<Sqlite>, view_name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DROP TABLE {view_name}")).execute(pool).await?;
    Ok(())
}

/// Creates the two `human_id` indexes (ADR 0032) a human-id-bearing view table needs: an equality
/// index serving `find_view_by_human_id` and the `list_views` ordering, and a
/// `(length(human_id), human_id)` index serving the `next_human_id` allocator's per-length-group
/// descending scan. Not `UNIQUE` — duplicate human ids are not prevented anywhere today.
///
/// `view_name` must be a trusted, code-supplied identifier (it is interpolated into the DDL).
///
/// # Errors
///
/// Returns the `sqlx` error if either `CREATE INDEX` statement fails.
#[cfg(feature = "sqlite")]
pub async fn create_sqlite_human_id_indexes(pool: &Pool<Sqlite>, view_name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!(
        "CREATE INDEX IF NOT EXISTS {view_name}_human_id_idx ON {view_name} (human_id)"
    ))
    .execute(pool)
    .await?;
    sqlx::query(&format!(
        "CREATE INDEX IF NOT EXISTS {view_name}_human_id_len_idx ON {view_name} (length(human_id), human_id)"
    ))
    .execute(pool)
    .await?;
    Ok(())
}

/// Deletes every row from a view (read-model) table, leaving the table itself in place.
///
/// Used by the projection rebuild (ADR 0010): a view is cleared, then replayed from the event log.
/// `view_name` must be a trusted, code-supplied identifier (it is interpolated into the statement).
///
/// # Errors
///
/// Returns the `sqlx` error if the `DELETE` statement fails.
#[cfg(feature = "sqlite")]
pub async fn clear_sqlite_view_table(pool: &Pool<Sqlite>, view_name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DELETE FROM {view_name}")).execute(pool).await?;
    Ok(())
}

/// The Postgres event log. The `postgres-es` repository expects `json` payload/metadata columns and
/// a `bigint` sequence; the `(aggregate_type, aggregate_id, sequence)` key is the concurrency guard.
#[cfg(feature = "postgres")]
const CREATE_EVENTS_TABLE_PG: &str = "
CREATE TABLE IF NOT EXISTS events (
    aggregate_type TEXT   NOT NULL,
    aggregate_id   TEXT   NOT NULL,
    sequence       BIGINT NOT NULL CHECK (sequence >= 0),
    event_type     TEXT   NOT NULL,
    event_version  TEXT   NOT NULL,
    payload        JSON   NOT NULL,
    metadata       JSON   NOT NULL,
    PRIMARY KEY (aggregate_type, aggregate_id, sequence)
)";

/// The optional Postgres snapshot store (unused by the event-sourced store, created for parity).
#[cfg(feature = "postgres")]
const CREATE_SNAPSHOTS_TABLE_PG: &str = "
CREATE TABLE IF NOT EXISTS snapshots (
    aggregate_type   TEXT   NOT NULL,
    aggregate_id     TEXT   NOT NULL,
    last_sequence    BIGINT NOT NULL CHECK (last_sequence >= 0),
    current_snapshot BIGINT NOT NULL CHECK (current_snapshot >= 0),
    payload          JSON   NOT NULL,
    PRIMARY KEY (aggregate_type, aggregate_id, last_sequence)
)";

/// Creates the core event-store tables on a fresh Postgres workspace database.
///
/// Idempotent (`IF NOT EXISTS`), safe to call on every open. Projection/view tables are created per
/// view with [`create_postgres_view_table`].
///
/// # Errors
///
/// Returns the `sqlx` error if a `CREATE TABLE` statement fails.
#[cfg(feature = "postgres")]
pub async fn init_postgres(pool: &Pool<Postgres>) -> Result<(), sqlx::Error> {
    sqlx::query(CREATE_EVENTS_TABLE_PG).execute(pool).await?;
    sqlx::query(CREATE_SNAPSHOTS_TABLE_PG).execute(pool).await?;
    Ok(())
}

/// Creates a `cqrs-es` view (read-model) table with the layout `PostgresViewRepository` expects:
/// `(view_id PRIMARY KEY, version, payload)` with a `json` payload, plus the generated `human_id`
/// column (ADR 0032) every view table carries; `postgres-es` writes explicit column lists, so the
/// extra column is invisible to it. `json_object_field_text` (what `->>` compiles to) is
/// `IMMUTABLE`, so no `::jsonb` cast is needed to make it a valid generated-column expression.
///
/// `view_name` must be a trusted, code-supplied identifier (it is interpolated into the DDL).
///
/// # Errors
///
/// Returns the `sqlx` error if the `CREATE TABLE` statement fails.
#[cfg(feature = "postgres")]
pub async fn create_postgres_view_table(pool: &Pool<Postgres>, view_name: &str) -> Result<(), sqlx::Error> {
    let ddl = format!(
        "CREATE TABLE IF NOT EXISTS {view_name} (
            view_id  TEXT   NOT NULL PRIMARY KEY,
            version  BIGINT NOT NULL,
            payload  JSON   NOT NULL,
            human_id text GENERATED ALWAYS AS (payload->'state'->>'human_id') STORED
        )"
    );
    sqlx::query(&ddl).execute(pool).await?;
    Ok(())
}

/// Returns whether `view_name` exists in Postgres but predates the generated `human_id` column
/// (ADR 0032) — the twin of [`sqlite_view_table_is_stale`]. A nonexistent table (nothing to
/// migrate; the caller creates it fresh) is not stale.
///
/// # Errors
///
/// Returns the `sqlx` error if the `information_schema` query fails.
#[cfg(feature = "postgres")]
pub async fn postgres_view_table_is_stale(pool: &Pool<Postgres>, view_name: &str) -> Result<bool, sqlx::Error> {
    let row = sqlx::query(
        "SELECT EXISTS (SELECT 1 FROM information_schema.tables WHERE table_name = $1) AS table_exists, \
         EXISTS (SELECT 1 FROM information_schema.columns \
         WHERE table_name = $1 AND column_name = 'human_id') AS has_human_id",
    )
    .bind(view_name)
    .fetch_one(pool)
    .await?;
    let table_exists: bool = row.get("table_exists");
    let has_human_id: bool = row.get("has_human_id");
    Ok(table_exists && !has_human_id)
}

/// Drops a Postgres view (read-model) table outright — the twin of [`drop_sqlite_view_table`].
///
/// `view_name` must be a trusted, code-supplied identifier (it is interpolated into the DDL).
///
/// # Errors
///
/// Returns the `sqlx` error if the `DROP TABLE` statement fails.
#[cfg(feature = "postgres")]
pub async fn drop_postgres_view_table(pool: &Pool<Postgres>, view_name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DROP TABLE {view_name}")).execute(pool).await?;
    Ok(())
}

/// Creates the two `human_id` indexes (ADR 0032) a human-id-bearing view table needs — the twin of
/// [`create_sqlite_human_id_indexes`]. The second key is `COLLATE "C"` so it matches the collation
/// the `next_human_id` allocator's `ORDER BY` uses (Postgres only picks an index when the query's
/// collation matches the index's); the plain equality index stays uncollated so `list_views`'
/// default-collation `ORDER BY human_id` still uses it. Not `UNIQUE`, for the same reason as the
/// SQLite twin.
///
/// `view_name` must be a trusted, code-supplied identifier (it is interpolated into the DDL).
///
/// # Errors
///
/// Returns the `sqlx` error if either `CREATE INDEX` statement fails.
#[cfg(feature = "postgres")]
pub async fn create_postgres_human_id_indexes(pool: &Pool<Postgres>, view_name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!(
        "CREATE INDEX IF NOT EXISTS {view_name}_human_id_idx ON {view_name} (human_id)"
    ))
    .execute(pool)
    .await?;
    sqlx::query(&format!(
        "CREATE INDEX IF NOT EXISTS {view_name}_human_id_len_idx ON {view_name} (length(human_id), human_id COLLATE \"C\")"
    ))
    .execute(pool)
    .await?;
    Ok(())
}

/// Deletes every row from a Postgres view (read-model) table, leaving the table itself in place.
///
/// Used by the projection rebuild (ADR 0010): a view is cleared, then replayed from the event log.
/// `view_name` must be a trusted, code-supplied identifier (it is interpolated into the statement).
///
/// # Errors
///
/// Returns the `sqlx` error if the `DELETE` statement fails.
#[cfg(feature = "postgres")]
pub async fn clear_postgres_view_table(pool: &Pool<Postgres>, view_name: &str) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DELETE FROM {view_name}")).execute(pool).await?;
    Ok(())
}
