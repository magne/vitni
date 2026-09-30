//! The Postgres half of the `match_keys` index — see the [module header](super).
//!
//! A function-for-function twin of [`sqlite`](super::sqlite). `key` is compared in the `"C"`
//! collation, so a prefix is a byte range the index serves whatever the database's locale.

use async_trait::async_trait;
use cqrs_es::{Aggregate, EventEnvelope, Query};
use sqlx::{Pool, Postgres, Row, Transaction};
use vitni_core::matching::{MatchableKind, Probe};

use super::{
    DirtyRecord, INSERT_CHUNK, KeyedRecord, MATCH_DIRTY_TABLE, MATCH_KEYS_STATE_TABLE, MATCH_KEYS_TABLE, kind_of,
    probe_condition,
};
use crate::store::DbError;

const CREATE_MATCH_KEYS_TABLE: &str = r#"
CREATE TABLE IF NOT EXISTS match_keys (
    kind         TEXT NOT NULL,
    key          TEXT COLLATE "C" NOT NULL,
    aggregate_id TEXT NOT NULL,
    PRIMARY KEY (kind, key, aggregate_id)
)"#;

const CREATE_MATCH_KEYS_AGGREGATE_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS match_keys_aggregate ON match_keys (kind, aggregate_id)";

const CREATE_MATCH_DIRTY_TABLE: &str = "
CREATE TABLE IF NOT EXISTS match_dirty (
    aggregate_type TEXT    NOT NULL,
    aggregate_id   TEXT    NOT NULL,
    generation     BIGINT  NOT NULL,
    PRIMARY KEY (aggregate_type, aggregate_id)
)";

const CREATE_MATCH_KEYS_STATE_TABLE: &str = "
CREATE TABLE IF NOT EXISTS match_keys_state (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    fingerprint TEXT    NOT NULL
)";

fn backend(action: &str) -> impl Fn(sqlx::Error) -> DbError + '_ {
    move |e| DbError::Backend(format!("{action}: {e}"))
}

/// Creates the three tables. Idempotent.
///
/// # Errors
///
/// Returns the `sqlx` error if a statement fails.
pub(crate) async fn create_tables(pool: &Pool<Postgres>) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_MATCH_KEYS_TABLE,
        CREATE_MATCH_KEYS_AGGREGATE_INDEX,
        CREATE_MATCH_DIRTY_TABLE,
        CREATE_MATCH_KEYS_STATE_TABLE,
    ] {
        sqlx::query(statement).execute(pool).await?;
    }
    Ok(())
}

/// Forgets what the index was built under, so the app layer rebuilds it on next use — the step a
/// projection rebuild takes, since keys need the packs this crate does not have.
///
/// # Errors
///
/// A [`DbError`] if the statement fails.
pub(crate) async fn clear_state(pool: &Pool<Postgres>) -> Result<(), DbError> {
    sqlx::query(&format!("DELETE FROM {MATCH_KEYS_STATE_TABLE}"))
        .execute(pool)
        .await
        .map_err(backend("clearing the match keys state"))?;
    Ok(())
}

/// A `cqrs-es` query marking every matchable aggregate a commit touches as dirty.
pub(crate) struct MatchDirtyQuery {
    pool: Pool<Postgres>,
}

impl MatchDirtyQuery {
    /// Wraps the pool.
    pub(crate) fn new(pool: Pool<Postgres>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl<A: Aggregate> Query<A> for MatchDirtyQuery {
    async fn dispatch(&self, aggregate_id: &str, events: &[EventEnvelope<A>]) {
        if events.is_empty() || MatchableKind::parse(A::TYPE).is_none() {
            return;
        }
        let result = sqlx::query(&format!(
            "INSERT INTO {MATCH_DIRTY_TABLE} (aggregate_type, aggregate_id, generation) VALUES ($1, $2, 1) \
             ON CONFLICT (aggregate_type, aggregate_id) DO UPDATE SET generation = {MATCH_DIRTY_TABLE}.generation + 1"
        ))
        .bind(A::TYPE)
        .bind(aggregate_id)
        .execute(&self.pool)
        .await;
        if let Err(error) = result {
            tracing::error!(aggregate_type = A::TYPE, aggregate_id, %error, "failed to mark a record for rekeying");
        }
    }
}

/// The fingerprint the index was built under, or `None` when it must be built.
///
/// # Errors
///
/// A [`DbError`] on a read failure.
pub(crate) async fn fingerprint(pool: &Pool<Postgres>) -> Result<Option<String>, DbError> {
    let row = sqlx::query(&format!(
        "SELECT fingerprint FROM {MATCH_KEYS_STATE_TABLE} WHERE id = 1"
    ))
    .fetch_optional(pool)
    .await
    .map_err(backend("reading the match keys state"))?;
    Ok(row.map(|row| row.get("fingerprint")))
}

/// Every record touched since it was keyed.
///
/// # Errors
///
/// A [`DbError`] on a read failure.
pub(crate) async fn dirty(pool: &Pool<Postgres>) -> Result<Vec<DirtyRecord>, DbError> {
    let rows = sqlx::query(&format!(
        "SELECT aggregate_type, aggregate_id, generation FROM {MATCH_DIRTY_TABLE} ORDER BY aggregate_type, aggregate_id"
    ))
    .fetch_all(pool)
    .await
    .map_err(backend("reading the dirty records"))?;
    let mut records = Vec::with_capacity(rows.len());
    for row in rows {
        records.push(DirtyRecord {
            kind: kind_of(row.get("aggregate_type"))?,
            aggregate_id: row.get("aggregate_id"),
            generation: row.get("generation"),
        });
    }
    Ok(records)
}

/// Replaces the keys of `records`, and clears `cleared` at the generations read, in one transaction.
///
/// # Errors
///
/// A [`DbError`] if a statement fails; nothing is written then.
pub(crate) async fn rekey(
    pool: &Pool<Postgres>,
    records: &[KeyedRecord],
    cleared: &[DirtyRecord],
) -> Result<(), DbError> {
    let mut tx = pool.begin().await.map_err(backend("starting a rekey"))?;
    for record in records {
        sqlx::query(&format!(
            "DELETE FROM {MATCH_KEYS_TABLE} WHERE kind = $1 AND aggregate_id = $2"
        ))
        .bind(record.kind.as_str())
        .bind(&record.aggregate_id)
        .execute(&mut *tx)
        .await
        .map_err(backend("deleting a record's match keys"))?;
    }
    insert(&mut tx, records).await?;
    clear_dirty(&mut tx, cleared).await?;
    tx.commit().await.map_err(backend("committing a rekey"))
}

/// Replaces the whole index with `records` built under `fingerprint`, and clears `cleared` at the
/// generations read, in one transaction — unless another rebuild already built it under
/// `fingerprint`, when nothing is written.
///
/// # Errors
///
/// A [`DbError`] if a statement fails; nothing is written then.
pub(crate) async fn reset(
    pool: &Pool<Postgres>,
    fingerprint: &str,
    records: &[KeyedRecord],
    cleared: &[DirtyRecord],
) -> Result<(), DbError> {
    let mut tx = pool.begin().await.map_err(backend("starting a match keys rebuild"))?;
    let built = sqlx::query(&format!(
        "SELECT fingerprint FROM {MATCH_KEYS_STATE_TABLE} WHERE id = 1"
    ))
    .fetch_optional(&mut *tx)
    .await
    .map_err(backend("reading the match keys state"))?
    .map(|row| row.get::<String, _>("fingerprint"));
    if built.as_deref() == Some(fingerprint) {
        // Another rebuild under the same rules finished first; the records its snapshot missed are
        // still dirty, while this one's may be older than its.
        return tx
            .commit()
            .await
            .map_err(backend("ending a superseded match keys rebuild"));
    }
    sqlx::query(&format!("DELETE FROM {MATCH_KEYS_TABLE}"))
        .execute(&mut *tx)
        .await
        .map_err(backend("clearing the match keys"))?;
    insert(&mut tx, records).await?;
    clear_dirty(&mut tx, cleared).await?;
    sqlx::query(&format!(
        "INSERT INTO {MATCH_KEYS_STATE_TABLE} (id, fingerprint) VALUES (1, $1) \
         ON CONFLICT (id) DO UPDATE SET fingerprint = EXCLUDED.fingerprint"
    ))
    .bind(fingerprint)
    .execute(&mut *tx)
    .await
    .map_err(backend("recording the match keys state"))?;
    tx.commit().await.map_err(backend("committing a match keys rebuild"))
}

async fn insert(tx: &mut Transaction<'_, Postgres>, records: &[KeyedRecord]) -> Result<(), DbError> {
    let mut rows: Vec<(&str, &str, &str)> = Vec::new();
    for record in records {
        for key in &record.keys {
            rows.push((record.kind.as_str(), key, &record.aggregate_id));
        }
    }
    for chunk in rows.chunks(INSERT_CHUNK) {
        let mut values = Vec::with_capacity(chunk.len());
        for row in 0..chunk.len() {
            values.push(format!("(${}, ${}, ${})", 3 * row + 1, 3 * row + 2, 3 * row + 3));
        }
        let sql = format!(
            "INSERT INTO {MATCH_KEYS_TABLE} (kind, key, aggregate_id) VALUES {} ON CONFLICT DO NOTHING",
            values.join(", ")
        );
        let mut query = sqlx::query(&sql);
        for (kind, key, id) in chunk {
            query = query.bind(*kind).bind(*key).bind(*id);
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(backend("inserting match keys"))?;
    }
    Ok(())
}

/// Clears each of `cleared` at the generation read. A row at another generation, or already gone,
/// means another commit or another refresh reached the record meanwhile, and the keys just written may
/// be older than theirs: the record is marked dirty again, so the next lookup rekeys it from fresh
/// views rather than trusting keys whose views it cannot vouch for.
async fn clear_dirty(tx: &mut Transaction<'_, Postgres>, cleared: &[DirtyRecord]) -> Result<(), DbError> {
    for record in cleared {
        let deleted = sqlx::query(&format!(
            "DELETE FROM {MATCH_DIRTY_TABLE} WHERE aggregate_type = $1 AND aggregate_id = $2 AND generation = $3"
        ))
        .bind(record.kind.as_str())
        .bind(&record.aggregate_id)
        .bind(record.generation)
        .execute(&mut **tx)
        .await
        .map_err(backend("clearing a dirty record"))?
        .rows_affected();
        if deleted == 0 {
            sqlx::query(&format!(
                "INSERT INTO {MATCH_DIRTY_TABLE} (aggregate_type, aggregate_id, generation) VALUES ($1, $2, 1) \
                 ON CONFLICT (aggregate_type, aggregate_id) DO UPDATE SET generation = {MATCH_DIRTY_TABLE}.generation + 1"
            ))
            .bind(record.kind.as_str())
            .bind(&record.aggregate_id)
            .execute(&mut **tx)
            .await
            .map_err(backend("marking a record dirty again"))?;
        }
    }
    Ok(())
}

/// The ids of every record of `kind` holding a key `probe` meets, in id order.
///
/// # Errors
///
/// A [`DbError`] on a read failure.
pub(crate) async fn candidates(
    pool: &Pool<Postgres>,
    kind: MatchableKind,
    probe: &Probe,
) -> Result<Vec<String>, DbError> {
    let Some((condition, values)) = probe_condition(probe, 1, |i| format!("${i}")) else {
        return Ok(Vec::new());
    };
    let sql = format!(
        "SELECT DISTINCT aggregate_id FROM {MATCH_KEYS_TABLE} WHERE kind = $1 AND {condition} ORDER BY aggregate_id"
    );
    let mut query = sqlx::query(&sql).bind(kind.as_str());
    for value in &values {
        query = query.bind(value);
    }
    let rows = query
        .fetch_all(pool)
        .await
        .map_err(backend("reading match candidates"))?;
    Ok(rows.into_iter().map(|row| row.get("aggregate_id")).collect())
}

/// Every `(aggregate_id, key)` of `kind`, by id then key.
///
/// # Errors
///
/// A [`DbError`] on a read failure.
pub(crate) async fn keys_of_kind(pool: &Pool<Postgres>, kind: MatchableKind) -> Result<Vec<(String, String)>, DbError> {
    let rows = sqlx::query(&format!(
        "SELECT aggregate_id, key FROM {MATCH_KEYS_TABLE} WHERE kind = $1 ORDER BY aggregate_id, key"
    ))
    .bind(kind.as_str())
    .fetch_all(pool)
    .await
    .map_err(backend("reading match keys"))?;
    Ok(rows
        .into_iter()
        .map(|row| (row.get("aggregate_id"), row.get("key")))
        .collect())
}
