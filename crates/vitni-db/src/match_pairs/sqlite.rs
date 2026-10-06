//! The SQLite half of the `match_pairs` projection — see the [module header](super).

use sqlx::{Pool, Row, Sqlite, Transaction};
use vitni_core::matching::{MatchBand, MatchableKind};

use super::{
    INSERT_CHUNK, MATCH_PAIRS_DIRTY_TABLE, MATCH_PAIRS_STATE_TABLE, MATCH_PAIRS_TABLE, MatchPair, PairRefresh,
    PairScope, band_of, band_rank, count_query, kind_of, list_query,
};
use crate::match_keys::DirtyRecord;
use crate::match_keys::sqlite::clear_dirty;
use crate::store::DbError;

const CREATE_MATCH_PAIRS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS match_pairs (
    kind  TEXT    NOT NULL,
    a     TEXT    NOT NULL,
    b     TEXT    NOT NULL,
    band  INTEGER NOT NULL,
    score REAL    NOT NULL,
    PRIMARY KEY (kind, a, b)
) WITHOUT ROWID";

const CREATE_MATCH_PAIRS_B_INDEX: &str = "CREATE INDEX IF NOT EXISTS match_pairs_by_b ON match_pairs (kind, b)";

const CREATE_MATCH_PAIRS_RANK_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS match_pairs_by_rank ON match_pairs (band DESC, score DESC)";

const CREATE_MATCH_PAIRS_DIRTY_TABLE: &str = "
CREATE TABLE IF NOT EXISTS match_pairs_dirty (
    aggregate_type TEXT    NOT NULL,
    aggregate_id   TEXT    NOT NULL,
    generation     INTEGER NOT NULL,
    PRIMARY KEY (aggregate_type, aggregate_id)
)";

const CREATE_MATCH_PAIRS_STATE_TABLE: &str = "
CREATE TABLE IF NOT EXISTS match_pairs_state (
    id          INTEGER PRIMARY KEY CHECK (id = 1),
    fingerprint TEXT    NOT NULL
)";

fn backend(action: &str) -> impl Fn(sqlx::Error) -> DbError + '_ {
    move |e| DbError::Backend(format!("{action}: {e}"))
}

fn placeholder(i: usize) -> String {
    format!("?{i}")
}

/// Creates the three tables. Idempotent.
///
/// # Errors
///
/// Returns the `sqlx` error if a statement fails.
pub(crate) async fn create_tables(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_MATCH_PAIRS_TABLE,
        CREATE_MATCH_PAIRS_B_INDEX,
        CREATE_MATCH_PAIRS_RANK_INDEX,
        CREATE_MATCH_PAIRS_DIRTY_TABLE,
        CREATE_MATCH_PAIRS_STATE_TABLE,
    ] {
        sqlx::query(statement).execute(pool).await?;
    }
    Ok(())
}

/// Forgets what the pairs were scored under, so the app layer scores them again on next use — the step
/// a projection rebuild takes.
///
/// # Errors
///
/// A [`DbError`] if the statement fails.
pub(crate) async fn clear_state(pool: &Pool<Sqlite>) -> Result<(), DbError> {
    sqlx::query(&format!("DELETE FROM {MATCH_PAIRS_STATE_TABLE}"))
        .execute(pool)
        .await
        .map_err(backend("clearing the match pairs state"))?;
    Ok(())
}

/// The fingerprint the pairs were scored under, or `None` when they must be scored.
///
/// # Errors
///
/// A [`DbError`] on a read failure.
pub(crate) async fn fingerprint(pool: &Pool<Sqlite>) -> Result<Option<String>, DbError> {
    let row = sqlx::query(&format!(
        "SELECT fingerprint FROM {MATCH_PAIRS_STATE_TABLE} WHERE id = 1"
    ))
    .fetch_optional(pool)
    .await
    .map_err(backend("reading the match pairs state"))?;
    Ok(row.map(|row| row.get("fingerprint")))
}

/// Every record touched since its pairs were refreshed.
///
/// # Errors
///
/// A [`DbError`] on a read failure.
pub(crate) async fn dirty(pool: &Pool<Sqlite>) -> Result<Vec<DirtyRecord>, DbError> {
    crate::match_keys::sqlite::dirty(MATCH_PAIRS_DIRTY_TABLE, pool).await
}

/// Replaces the pairs each of `refreshes` covers, and clears `cleared` at the generations read, in one
/// transaction.
///
/// # Errors
///
/// A [`DbError`] if a statement fails; nothing is written then.
pub(crate) async fn refresh(
    pool: &Pool<Sqlite>,
    refreshes: &[PairRefresh],
    cleared: &[DirtyRecord],
) -> Result<(), DbError> {
    let mut tx = pool.begin().await.map_err(backend("starting a match pairs refresh"))?;
    for refresh in refreshes {
        delete_scope(&mut tx, refresh.kind, &refresh.scope).await?;
        insert(&mut tx, &refresh.pairs).await?;
    }
    clear_dirty(&mut tx, MATCH_PAIRS_DIRTY_TABLE, cleared).await?;
    tx.commit().await.map_err(backend("committing a match pairs refresh"))
}

/// Replaces every pair with `pairs`, scored under `fingerprint`, and clears `cleared` at the generations
/// read, in one transaction — unless another rebuild already scored them under `fingerprint`, when
/// nothing is written.
///
/// # Errors
///
/// A [`DbError`] if a statement fails; nothing is written then.
pub(crate) async fn reset(
    pool: &Pool<Sqlite>,
    fingerprint: &str,
    pairs: &[MatchPair],
    cleared: &[DirtyRecord],
) -> Result<(), DbError> {
    let mut tx = pool.begin().await.map_err(backend("starting a match pairs rebuild"))?;
    let built = sqlx::query(&format!(
        "SELECT fingerprint FROM {MATCH_PAIRS_STATE_TABLE} WHERE id = 1"
    ))
    .fetch_optional(&mut *tx)
    .await
    .map_err(backend("reading the match pairs state"))?
    .map(|row| row.get::<String, _>("fingerprint"));
    if built.as_deref() == Some(fingerprint) {
        // Another rebuild under the same rules finished first; the records its snapshot missed are
        // still dirty, while this one's may be older than its.
        return tx
            .commit()
            .await
            .map_err(backend("ending a superseded match pairs rebuild"));
    }
    sqlx::query(&format!("DELETE FROM {MATCH_PAIRS_TABLE}"))
        .execute(&mut *tx)
        .await
        .map_err(backend("clearing the match pairs"))?;
    insert(&mut tx, pairs).await?;
    clear_dirty(&mut tx, MATCH_PAIRS_DIRTY_TABLE, cleared).await?;
    sqlx::query(&format!(
        "INSERT INTO {MATCH_PAIRS_STATE_TABLE} (id, fingerprint) VALUES (1, ?) \
         ON CONFLICT (id) DO UPDATE SET fingerprint = excluded.fingerprint"
    ))
    .bind(fingerprint)
    .execute(&mut *tx)
    .await
    .map_err(backend("recording the match pairs state"))?;
    tx.commit().await.map_err(backend("committing a match pairs rebuild"))
}

/// Deletes the stored pairs of `kind` that `scope` covers.
async fn delete_scope(tx: &mut Transaction<'_, Sqlite>, kind: MatchableKind, scope: &PairScope) -> Result<(), DbError> {
    match scope {
        PairScope::Kind => {
            sqlx::query(&format!("DELETE FROM {MATCH_PAIRS_TABLE} WHERE kind = ?"))
                .bind(kind.as_str())
                .execute(&mut **tx)
                .await
                .map_err(backend("clearing a kind's match pairs"))?;
        }
        PairScope::Records(ids) => {
            let ids = serde_json::to_string(ids).map_err(|e| DbError::Backend(e.to_string()))?;
            sqlx::query(&format!(
                "DELETE FROM {MATCH_PAIRS_TABLE} WHERE kind = ?1 \
                 AND (a IN (SELECT value FROM json_each(?2)) OR b IN (SELECT value FROM json_each(?2)))"
            ))
            .bind(kind.as_str())
            .bind(ids)
            .execute(&mut **tx)
            .await
            .map_err(backend("clearing records' match pairs"))?;
        }
    }
    Ok(())
}

async fn insert(tx: &mut Transaction<'_, Sqlite>, pairs: &[MatchPair]) -> Result<(), DbError> {
    for chunk in pairs.chunks(INSERT_CHUNK) {
        let values = vec!["(?, ?, ?, ?, ?)"; chunk.len()].join(", ");
        let sql = format!("INSERT OR REPLACE INTO {MATCH_PAIRS_TABLE} (kind, a, b, band, score) VALUES {values}");
        let mut query = sqlx::query(&sql);
        for pair in chunk {
            query = query
                .bind(pair.kind.as_str())
                .bind(&pair.a)
                .bind(&pair.b)
                .bind(band_rank(pair.band))
                .bind(pair.score);
        }
        query
            .execute(&mut **tx)
            .await
            .map_err(backend("inserting match pairs"))?;
    }
    Ok(())
}

/// The undecided pairs from `min_band` up, of `kinds`, strongest first, at most `limit`.
///
/// # Errors
///
/// A [`DbError`] on a read failure.
pub(crate) async fn pairs(
    pool: &Pool<Sqlite>,
    kinds: &[MatchableKind],
    min_band: MatchBand,
    limit: Option<usize>,
) -> Result<Vec<MatchPair>, DbError> {
    if kinds.is_empty() {
        return Ok(Vec::new());
    }
    let sql = list_query(placeholder, kinds.len(), limit);
    let mut query = sqlx::query(&sql).bind(band_rank(min_band));
    for kind in kinds {
        query = query.bind(kind.as_str());
    }
    let rows = query
        .fetch_all(pool)
        .await
        .map_err(backend("reading the match pairs"))?;
    let mut pairs = Vec::with_capacity(rows.len());
    for row in rows {
        pairs.push(MatchPair {
            kind: kind_of(row.get("kind"))?,
            a: row.get("a"),
            b: row.get("b"),
            band: band_of(row.get("band"))?,
            score: row.get("score"),
        });
    }
    Ok(pairs)
}

/// How many undecided pairs from `min_band` up each kind holds, for the kinds holding any, in kind
/// name order.
///
/// # Errors
///
/// A [`DbError`] on a read failure.
pub(crate) async fn counts(pool: &Pool<Sqlite>, min_band: MatchBand) -> Result<Vec<(MatchableKind, usize)>, DbError> {
    let rows = sqlx::query(&count_query(placeholder))
        .bind(band_rank(min_band))
        .fetch_all(pool)
        .await
        .map_err(backend("counting the match pairs"))?;
    let mut counts = Vec::with_capacity(rows.len());
    for row in rows {
        let n: i64 = row.get("n");
        let n = usize::try_from(n).map_err(|e| DbError::Backend(e.to_string()))?;
        counts.push((kind_of(row.get("kind"))?, n));
    }
    Ok(counts)
}
