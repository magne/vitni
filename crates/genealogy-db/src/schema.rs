//! Initial table creation for a workspace database (ADR 0002: `genealogy-db` owns the schema).
//!
//! `cqrs-es` backends expect an `events` table (and a `snapshots` table) with a fixed column
//! layout; the SQLite backend ships no migrations, so we own the DDL here. The column shape
//! mirrors what `sqlite-es` reads and writes: `(aggregate_type, aggregate_id, sequence,
//! event_type, event_version, payload, metadata)` keyed by `(aggregate_type, aggregate_id,
//! sequence)` — the `UNIQUE` key that *is* the optimistic-concurrency guard.

#[cfg(feature = "sqlite")]
use sqlx::{Pool, Sqlite};

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
/// `(view_id PRIMARY KEY, version, payload)`.
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
            view_id TEXT    NOT NULL PRIMARY KEY,
            version INTEGER NOT NULL,
            payload TEXT    NOT NULL
        )"
    );
    sqlx::query(&ddl).execute(pool).await?;
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
