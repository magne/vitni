//! The SQLite half of the `record_origins` index — see the [module header](super).

use std::collections::BTreeSet;

use async_trait::async_trait;
use cqrs_es::{Aggregate, EventEnvelope, Query};
use serde::Serialize;
use sqlx::{Pool, Row, Sqlite};
use vitni_core::assertions::{Envelope, EventBody};
use vitni_core::ids::AssertionId;
use vitni_core::origin::{ContentDigest, DatasetId, RecordOrigin};

use super::{
    IndexRow, OriginResolution, OriginRow, PendingWrite, RECORD_ORIGINS_PENDING_TABLE, RECORD_ORIGINS_STATE_TABLE,
    RECORD_ORIGINS_TABLE, created_key, decode_assertion_id, decode_run, decode_timestamp, index_rows, resolved_key,
};
use crate::store::DbError;

const CREATE_RECORD_ORIGINS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS record_origins (
    id              INTEGER PRIMARY KEY,
    dataset         TEXT    NOT NULL,
    record          TEXT    NOT NULL,
    item            TEXT,
    field_key       TEXT    NOT NULL,
    aggregate_kind  TEXT    NOT NULL,
    aggregate_id    TEXT    NOT NULL,
    assertion_id    TEXT    NOT NULL,
    digest          TEXT    NOT NULL,
    run             TEXT    NOT NULL,
    occurred_at     TEXT    NOT NULL,
    live            INTEGER NOT NULL,
    event_key       TEXT    NOT NULL
)";

const CREATE_RECORD_ORIGINS_KEY_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS record_origins_key ON record_origins (dataset, record, item, field_key)";

const CREATE_RECORD_ORIGINS_AGGREGATE_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS record_origins_aggregate ON record_origins (aggregate_kind, aggregate_id)";

const CREATE_RECORD_ORIGINS_EVENT_INDEX: &str =
    "CREATE UNIQUE INDEX IF NOT EXISTS record_origins_event ON record_origins (event_key)";

const CREATE_RECORD_ORIGINS_PENDING_TABLE: &str = "
CREATE TABLE IF NOT EXISTS record_origins_pending (
    id             INTEGER PRIMARY KEY,
    aggregate_type TEXT    NOT NULL,
    aggregate_id   TEXT    NOT NULL
)";

const CREATE_RECORD_ORIGINS_STATE_TABLE: &str = "
CREATE TABLE IF NOT EXISTS record_origins_state (
    id INTEGER PRIMARY KEY CHECK (id = 1)
)";

/// Creates the index table, its lookups, its completion marker and its write journal. Idempotent. A
/// table from before rows carried their event's key is dropped and recreated empty. Returns whether
/// the index needs filling from the event log: it is new, or the last fill or rebuild never finished.
///
/// # Errors
///
/// Returns the `sqlx` error if a statement fails.
pub(crate) async fn create_tables(pool: &Pool<Sqlite>) -> Result<bool, sqlx::Error> {
    sqlx::query(CREATE_RECORD_ORIGINS_STATE_TABLE).execute(pool).await?;
    sqlx::query(CREATE_RECORD_ORIGINS_PENDING_TABLE).execute(pool).await?;
    sqlx::query(CREATE_RECORD_ORIGINS_TABLE).execute(pool).await?;
    let keyed = sqlx::query("SELECT 1 FROM pragma_table_info('record_origins') WHERE name = 'event_key'")
        .fetch_optional(pool)
        .await?
        .is_some();
    if !keyed {
        sqlx::query(&format!("DELETE FROM {RECORD_ORIGINS_STATE_TABLE}"))
            .execute(pool)
            .await?;
        sqlx::query(&format!("DROP TABLE {RECORD_ORIGINS_TABLE}"))
            .execute(pool)
            .await?;
        sqlx::query(CREATE_RECORD_ORIGINS_TABLE).execute(pool).await?;
    }
    sqlx::query(CREATE_RECORD_ORIGINS_KEY_INDEX).execute(pool).await?;
    sqlx::query(CREATE_RECORD_ORIGINS_AGGREGATE_INDEX).execute(pool).await?;
    sqlx::query(CREATE_RECORD_ORIGINS_EVENT_INDEX).execute(pool).await?;
    let complete = sqlx::query(&format!("SELECT 1 FROM {RECORD_ORIGINS_STATE_TABLE}"))
        .fetch_optional(pool)
        .await?
        .is_some();
    Ok(!complete)
}

/// Locks the index for a refill inside the caller's transaction, then reads whether it is complete
/// after all — another process may have filled it while this one waited.
///
/// A write, so SQLite takes its one write lock here: a second process opening the workspace waits on
/// it (up to the busy timeout), and so does any commit, whose index row the refill would otherwise miss.
///
/// # Errors
///
/// A [`DbError`] if a statement fails.
pub(crate) async fn lock_for_refill(pool: &Pool<Sqlite>) -> Result<bool, DbError> {
    sqlx::query(&format!("UPDATE {RECORD_ORIGINS_STATE_TABLE} SET id = id"))
        .execute(pool)
        .await
        .map_err(|e| DbError::Backend(format!("locking record origins: {e}")))?;
    let complete = sqlx::query(&format!("SELECT 1 FROM {RECORD_ORIGINS_STATE_TABLE}"))
        .fetch_optional(pool)
        .await
        .map_err(|e| DbError::Backend(format!("reading the record origins marker: {e}")))?
        .is_some();
    Ok(complete)
}

/// Records that the index now holds every originated event in the log.
///
/// # Errors
///
/// A [`DbError`] if the statement fails.
pub(crate) async fn mark_complete(pool: &Pool<Sqlite>) -> Result<(), DbError> {
    sqlx::query(&format!(
        "INSERT INTO {RECORD_ORIGINS_STATE_TABLE} (id) VALUES (1) ON CONFLICT DO NOTHING"
    ))
    .execute(pool)
    .await
    .map_err(|e| DbError::Backend(format!("marking record origins complete: {e}")))?;
    Ok(())
}

/// Withdraws [`mark_complete`] before the index is cleared, so a rebuild cut short is redone on the
/// next open.
///
/// # Errors
///
/// A [`DbError`] if the statement fails.
pub(crate) async fn mark_incomplete(pool: &Pool<Sqlite>) -> Result<(), DbError> {
    sqlx::query(&format!("DELETE FROM {RECORD_ORIGINS_STATE_TABLE}"))
        .execute(pool)
        .await
        .map_err(|e| DbError::Backend(format!("marking record origins incomplete: {e}")))?;
    Ok(())
}

/// Journals a command about to run on the aggregate, before its events commit; returns the entry
/// for [`end_write`].
///
/// # Errors
///
/// A [`DbError`] if the statement fails.
pub(crate) async fn begin_write(pool: &Pool<Sqlite>, aggregate_type: &str, aggregate_id: &str) -> Result<i64, DbError> {
    sqlx::query_scalar(&format!(
        "INSERT INTO {RECORD_ORIGINS_PENDING_TABLE} (aggregate_type, aggregate_id) VALUES (?, ?) RETURNING id"
    ))
    .bind(aggregate_type)
    .bind(aggregate_id)
    .fetch_one(pool)
    .await
    .map_err(|e| DbError::Backend(format!("journalling a write to {aggregate_type} {aggregate_id}: {e}")))
}

/// Withdraws a [`begin_write`] entry once its command's events and index rows are written.
///
/// # Errors
///
/// A [`DbError`] if the statement fails.
pub(crate) async fn end_write(pool: &Pool<Sqlite>, entry: i64) -> Result<(), DbError> {
    sqlx::query(&format!("DELETE FROM {RECORD_ORIGINS_PENDING_TABLE} WHERE id = ?"))
        .bind(entry)
        .execute(pool)
        .await
        .map_err(|e| DbError::Backend(format!("ending a journalled write: {e}")))?;
    Ok(())
}

/// Every journal entry left by a command that never finished.
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn pending_writes(pool: &Pool<Sqlite>) -> Result<Vec<PendingWrite>, DbError> {
    let rows = sqlx::query(&format!(
        "SELECT id, aggregate_type, aggregate_id FROM {RECORD_ORIGINS_PENDING_TABLE} ORDER BY id"
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Backend(format!("reading the write journal: {e}")))?;
    let mut writes = Vec::with_capacity(rows.len());
    for row in rows {
        writes.push(PendingWrite {
            entry: row.get("id"),
            aggregate_type: row.get("aggregate_type"),
            aggregate_id: row.get("aggregate_id"),
        });
    }
    Ok(writes)
}

/// Deletes every row — the rebuild's clearing step (ADR 0010).
///
/// # Errors
///
/// A [`DbError`] if the statement fails.
pub(crate) async fn clear_table(pool: &Pool<Sqlite>) -> Result<(), DbError> {
    sqlx::query(&format!("DELETE FROM {RECORD_ORIGINS_TABLE}"))
        .execute(pool)
        .await
        .map_err(|e| DbError::Backend(format!("clearing record origins: {e}")))?;
    Ok(())
}

/// A `cqrs-es` query that records every originated event of one aggregate kind, and re-reads the
/// aggregate's live assertions from its projection after a correction. Must be appended *after* the
/// aggregate's `GenericQuery`, so the projection it reads is already up to date.
pub(crate) struct RecordOriginsQuery {
    pool: Pool<Sqlite>,
    view_table: &'static str,
}

impl RecordOriginsQuery {
    /// Wraps the pool, and the projection table of the aggregate kind it indexes.
    pub(crate) fn new(pool: Pool<Sqlite>, view_table: &'static str) -> Self {
        Self { pool, view_table }
    }

    async fn index(&self, aggregate_type: &str, aggregate_id: &str, events: &[IndexRow]) -> Result<(), DbError> {
        for row in events {
            insert_row(&self.pool, row).await?;
        }
        if events.iter().any(|row| row.corrects) {
            refresh_live(&self.pool, self.view_table, aggregate_type, aggregate_id).await?;
        }
        Ok(())
    }
}

#[async_trait]
impl<A, B> Query<A> for RecordOriginsQuery
where
    A: Aggregate<Event = Envelope<B>>,
    B: EventBody + Serialize + Send + Sync,
{
    async fn dispatch(&self, aggregate_id: &str, events: &[EventEnvelope<A>]) {
        let result = match index_rows::<A, B>(aggregate_id, events) {
            Ok(rows) => self.index(A::TYPE, aggregate_id, &rows).await,
            Err(error) => Err(error),
        };
        if let Err(error) = result {
            tracing::error!(aggregate_type = A::TYPE, aggregate_id, %error, "failed to update the record origins index");
            // Journalled again, so the next open indexes the aggregate even after its command withdraws
            // its own entry.
            if let Err(error) = begin_write(&self.pool, A::TYPE, aggregate_id).await {
                tracing::error!(aggregate_type = A::TYPE, aggregate_id, %error, "failed to journal the aggregate for the next open");
            }
        }
    }
}

async fn insert_row(pool: &Pool<Sqlite>, row: &IndexRow) -> Result<(), DbError> {
    let Some(origin) = &row.origin else {
        return Ok(());
    };
    sqlx::query(&format!(
        "INSERT INTO {RECORD_ORIGINS_TABLE} (dataset, record, item, field_key, aggregate_kind, aggregate_id, \
         assertion_id, digest, run, occurred_at, live, event_key) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, 1, ?) ON CONFLICT (event_key) DO NOTHING"
    ))
    .bind(&origin.dataset)
    .bind(&origin.record)
    .bind(origin.item.as_deref())
    .bind(&origin.field_key)
    .bind(&origin.aggregate_kind)
    .bind(&origin.aggregate_id)
    .bind(&origin.assertion_id)
    .bind(&origin.digest)
    .bind(&origin.run)
    .bind(&origin.occurred_at)
    .bind(&origin.event_key)
    .execute(pool)
    .await
    .map_err(|e| DbError::Backend(format!("inserting a record origin: {e}")))?;
    Ok(())
}

/// Marks each of the aggregate's rows live exactly when its assertion is still in the projection's
/// `live_assertions`, so a retraction's cascades (a family member's child relationships) are
/// covered too.
async fn refresh_live(
    pool: &Pool<Sqlite>,
    view_table: &str,
    aggregate_type: &str,
    aggregate_id: &str,
) -> Result<(), DbError> {
    let sql =
        format!("SELECT json_extract(payload, '$.state.live_assertions') AS live FROM {view_table} WHERE view_id = ?");
    let row = sqlx::query(&sql)
        .bind(aggregate_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| DbError::Backend(format!("reading live assertions: {e}")))?;
    let Some(row) = row else {
        return Ok(());
    };
    let live: Option<String> = row.get("live");
    let live: BTreeSet<AssertionId> = match live {
        Some(json) => {
            serde_json::from_str(&json).map_err(|e| DbError::Backend(format!("decoding live assertions: {e}")))?
        }
        None => return Ok(()),
    };
    let rows = sqlx::query(&format!(
        "SELECT id, assertion_id FROM {RECORD_ORIGINS_TABLE} WHERE aggregate_kind = ? AND aggregate_id = ?"
    ))
    .bind(aggregate_type)
    .bind(aggregate_id)
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Backend(format!("reading record origins: {e}")))?;
    for row in rows {
        let id: i64 = row.get("id");
        let assertion_id: String = row.get("assertion_id");
        let is_live = live.iter().any(|live| live.to_string() == assertion_id);
        sqlx::query(&format!("UPDATE {RECORD_ORIGINS_TABLE} SET live = ? WHERE id = ?"))
            .bind(is_live)
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| DbError::Backend(format!("updating a record origin: {e}")))?;
    }
    Ok(())
}

/// Every row recorded for `(dataset, record, item)` under `field_key`, oldest first.
///
/// # Errors
///
/// A [`DbError`] if the query fails or a row does not decode.
pub(crate) async fn rows(
    pool: &Pool<Sqlite>,
    dataset: &str,
    record: &str,
    item: Option<&str>,
    field_key: &str,
) -> Result<Vec<OriginRow>, DbError> {
    let rows = sqlx::query(&format!(
        "SELECT field_key, aggregate_kind, aggregate_id, assertion_id, digest, occurred_at, live \
         FROM {RECORD_ORIGINS_TABLE} WHERE dataset = ? AND record = ? AND item IS ? AND field_key = ? ORDER BY id"
    ))
    .bind(dataset)
    .bind(record)
    .bind(item)
    .bind(field_key)
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Backend(format!("reading record origins: {e}")))?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        out.push(OriginRow {
            field_key: row.get("field_key"),
            aggregate_kind: row.get("aggregate_kind"),
            aggregate_id: row.get("aggregate_id"),
            assertion_id: decode_assertion_id(&row.get::<String, _>("assertion_id"))?,
            digest: ContentDigest::new(row.get::<String, _>("digest")),
            occurred_at: decode_timestamp(&row.get::<String, _>("occurred_at"))?,
            live: row.get("live"),
        });
    }
    Ok(out)
}

/// Every row in the index, in insertion order — for tests comparing a live index to a rebuilt one.
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn all_rows(pool: &Pool<Sqlite>) -> Result<Vec<Vec<String>>, DbError> {
    let rows = sqlx::query(&format!(
        "SELECT dataset, record, COALESCE(item, ''), field_key, aggregate_kind, aggregate_id, assertion_id, digest, \
         run, occurred_at, CAST(live AS TEXT) FROM {RECORD_ORIGINS_TABLE} ORDER BY assertion_id, field_key, aggregate_id"
    ))
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Backend(format!("reading record origins: {e}")))?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let mut columns = Vec::with_capacity(11);
        for index in 0..11 {
            columns.push(row.get::<String, _>(index));
        }
        out.push(columns);
    }
    Ok(out)
}

/// What `(dataset, record, item)` resolves onto for `kind`: the aggregate its creating event made,
/// or the one a run recorded resolving it onto, whichever came last.
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn resolve(
    pool: &Pool<Sqlite>,
    dataset: &str,
    record: &str,
    item: Option<&str>,
    kind: &str,
) -> Result<Option<OriginResolution>, DbError> {
    let row = sqlx::query(&format!(
        "SELECT aggregate_id, field_key FROM {RECORD_ORIGINS_TABLE} WHERE dataset = ? AND record = ? AND item IS ? \
         AND aggregate_kind = ? AND field_key IN (?, ?) ORDER BY id DESC LIMIT 1"
    ))
    .bind(dataset)
    .bind(record)
    .bind(item)
    .bind(kind)
    .bind(super::created_key(kind))
    .bind(super::resolved_key(kind))
    .fetch_optional(pool)
    .await
    .map_err(|e| DbError::Backend(format!("resolving a record origin: {e}")))?;
    Ok(row.map(|row| OriginResolution {
        aggregate_id: row.get("aggregate_id"),
        created: row.get::<String, _>("field_key") == super::created_key(kind),
    }))
}

/// The origin of the creating event of every aggregate of `kind` that was imported, as
/// `(aggregate_id, origin)`, oldest first. The origin carries no content digest: the index holds the
/// digest of the event body, not the importer's.
///
/// # Errors
///
/// A [`DbError`] if the query fails or a row does not decode.
pub(crate) async fn created(pool: &Pool<Sqlite>, kind: &str) -> Result<Vec<(String, RecordOrigin)>, DbError> {
    let rows = sqlx::query(&format!(
        "SELECT aggregate_id, dataset, record, item, run FROM {RECORD_ORIGINS_TABLE} \
         WHERE aggregate_kind = ? AND field_key = ? ORDER BY id"
    ))
    .bind(kind)
    .bind(created_key(kind))
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Backend(format!("reading creating origins: {e}")))?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let origin = RecordOrigin {
            dataset: DatasetId::new(row.get::<String, _>("dataset")),
            record: row.get("record"),
            item: row.get("item"),
            digest: None,
            run: decode_run(&row.get::<String, _>("run"))?,
        };
        out.push((row.get("aggregate_id"), origin));
    }
    Ok(out)
}

/// The creating origin of the aggregate `aggregate_id` of `kind`, or `None` when no import created it
/// (ADR 0037 §4).
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn created_one(
    pool: &Pool<Sqlite>,
    kind: &str,
    aggregate_id: &str,
) -> Result<Option<RecordOrigin>, DbError> {
    let row = sqlx::query(&format!(
        "SELECT dataset, record, item, run FROM {RECORD_ORIGINS_TABLE} \
         WHERE aggregate_kind = ? AND field_key = ? AND aggregate_id = ? ORDER BY id LIMIT 1"
    ))
    .bind(kind)
    .bind(created_key(kind))
    .bind(aggregate_id)
    .fetch_optional(pool)
    .await
    .map_err(|e| DbError::Backend(format!("reading the creating origin of {aggregate_id}: {e}")))?;
    let Some(row) = row else {
        return Ok(None);
    };
    Ok(Some(RecordOrigin {
        dataset: DatasetId::new(row.get::<String, _>("dataset")),
        record: row.get("record"),
        item: row.get("item"),
        digest: None,
        run: decode_run(&row.get::<String, _>("run"))?,
    }))
}

/// The origin of every live assertion on the aggregate `aggregate_id` of `kind`, as `(assertion,
/// origin)` in the order they were asserted (ADR 0037 §2): what a claim's *Why we believe* names. A
/// recorded resolution is not an assertion on the aggregate and is left out. The origins carry no
/// content digest.
///
/// # Errors
///
/// A [`DbError`] if the query fails or a row does not decode.
pub(crate) async fn assertion_origins(
    pool: &Pool<Sqlite>,
    kind: &str,
    aggregate_id: &str,
) -> Result<Vec<(AssertionId, RecordOrigin)>, DbError> {
    let rows = sqlx::query(&format!(
        "SELECT assertion_id, dataset, record, item, run FROM {RECORD_ORIGINS_TABLE} \
         WHERE aggregate_kind = ? AND aggregate_id = ? AND field_key <> ? AND live = 1 ORDER BY id"
    ))
    .bind(kind)
    .bind(aggregate_id)
    .bind(resolved_key(kind))
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Backend(format!("reading the assertion origins of {aggregate_id}: {e}")))?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let origin = RecordOrigin {
            dataset: DatasetId::new(row.get::<String, _>("dataset")),
            record: row.get("record"),
            item: row.get("item"),
            digest: None,
            run: decode_run(&row.get::<String, _>("run"))?,
        };
        out.push((decode_assertion_id(&row.get::<String, _>("assertion_id"))?, origin));
    }
    Ok(out)
}

/// How many of `records` each dataset already holds as an aggregate of `kind` (ADR 0037 §3): records
/// its runs created or resolved, as `(dataset, count)` in dataset order. Only the record's own entity
/// counts, never one of its items.
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn overlap(
    pool: &Pool<Sqlite>,
    kind: &str,
    records: &[String],
) -> Result<Vec<(DatasetId, usize)>, DbError> {
    let records = serde_json::to_string(records).map_err(|e| DbError::Backend(format!("encoding record ids: {e}")))?;
    let rows = sqlx::query(&format!(
        "SELECT dataset, COUNT(DISTINCT record) AS shared FROM {RECORD_ORIGINS_TABLE} \
         WHERE aggregate_kind = ? AND field_key IN (?, ?) AND item IS NULL \
         AND record IN (SELECT value FROM json_each(?)) GROUP BY dataset ORDER BY dataset"
    ))
    .bind(kind)
    .bind(created_key(kind))
    .bind(resolved_key(kind))
    .bind(records)
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Backend(format!("counting a file's records per dataset: {e}")))?;
    let mut out = Vec::with_capacity(rows.len());
    for row in rows {
        let shared = usize::try_from(row.get::<i64, _>("shared")).unwrap_or(usize::MAX);
        out.push((DatasetId::new(row.get::<String, _>("dataset")), shared));
    }
    Ok(out)
}
