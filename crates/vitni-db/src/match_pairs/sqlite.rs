//! The SQLite half of the `match_pairs` projection — see the [module header](super).

use std::collections::BTreeMap;

use sqlx::{Pool, Row, Sqlite, Transaction};
use vitni_core::matching::{MatchBand, MatchableKind};

use super::{
    INSERT_CHUNK, MATCH_PAIR_COUNTS_TABLE, MATCH_PAIRS_DIRTY_TABLE, MATCH_PAIRS_STATE_TABLE, MATCH_PAIRS_TABLE,
    MatchPair, PairRefresh, PairScope, band_of, band_rank, decided_query, kind_of, list_query, stored_count_query,
    strongest, tally, undecided_counts,
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

// Finds a record's pairs by `b`, as the primary key does by `a`, with the band for a count's filter.
const CREATE_MATCH_PAIRS_B_INDEX: &str = "CREATE INDEX IF NOT EXISTS match_pairs_by_b ON match_pairs (kind, b, band)";

// A `WITHOUT ROWID` index carries the primary key after its own columns, so this one orders a kind's
// pairs by band, score, `a` and `b` — a list's whole `ORDER BY`.
const CREATE_MATCH_PAIRS_RANK_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS match_pairs_by_rank ON match_pairs (kind, band DESC, score DESC)";

const CREATE_MATCH_PAIR_COUNTS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS match_pair_counts (
    kind TEXT    NOT NULL,
    band INTEGER NOT NULL,
    n    INTEGER NOT NULL,
    PRIMARY KEY (kind, band)
)";

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

/// Creates the four tables. Idempotent.
///
/// # Errors
///
/// Returns the `sqlx` error if a statement fails.
pub(crate) async fn create_tables(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    for statement in [
        CREATE_MATCH_PAIRS_TABLE,
        CREATE_MATCH_PAIRS_B_INDEX,
        CREATE_MATCH_PAIRS_RANK_INDEX,
        CREATE_MATCH_PAIR_COUNTS_TABLE,
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
        count(&mut tx, tally(&refresh.pairs)).await?;
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
    for table in [MATCH_PAIRS_TABLE, MATCH_PAIR_COUNTS_TABLE] {
        sqlx::query(&format!("DELETE FROM {table}"))
            .execute(&mut *tx)
            .await
            .map_err(backend("clearing the match pairs"))?;
    }
    insert(&mut tx, pairs).await?;
    count(&mut tx, tally(pairs)).await?;
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

/// Deletes the stored pairs of `kind` that `scope` covers, and takes them off the kept counts.
async fn delete_scope(tx: &mut Transaction<'_, Sqlite>, kind: MatchableKind, scope: &PairScope) -> Result<(), DbError> {
    match scope {
        PairScope::Kind => {
            for table in [MATCH_PAIRS_TABLE, MATCH_PAIR_COUNTS_TABLE] {
                sqlx::query(&format!("DELETE FROM {table} WHERE kind = ?"))
                    .bind(kind.as_str())
                    .execute(&mut **tx)
                    .await
                    .map_err(backend("clearing a kind's match pairs"))?;
            }
        }
        PairScope::Records(ids) => {
            let ids = serde_json::to_string(ids).map_err(|e| DbError::Backend(e.to_string()))?;
            let rows = sqlx::query(&format!(
                "DELETE FROM {MATCH_PAIRS_TABLE} WHERE kind = ?1 \
                 AND (a IN (SELECT value FROM json_each(?2)) OR b IN (SELECT value FROM json_each(?2))) \
                 RETURNING band"
            ))
            .bind(kind.as_str())
            .bind(ids)
            .fetch_all(&mut **tx)
            .await
            .map_err(backend("clearing records' match pairs"))?;
            let mut deleted = BTreeMap::new();
            for row in rows {
                *deleted.entry((kind, row.get::<i64, _>("band"))).or_insert(0) -= 1;
            }
            count(tx, deleted).await?;
        }
    }
    Ok(())
}

/// Adds `deltas`, by kind and band, to the kept counts.
async fn count(tx: &mut Transaction<'_, Sqlite>, deltas: BTreeMap<(MatchableKind, i64), i64>) -> Result<(), DbError> {
    for ((kind, band), delta) in deltas {
        sqlx::query(&format!(
            "INSERT INTO {MATCH_PAIR_COUNTS_TABLE} (kind, band, n) VALUES (?, ?, ?) \
             ON CONFLICT (kind, band) DO UPDATE SET n = n + excluded.n"
        ))
        .bind(kind.as_str())
        .bind(band)
        .bind(delta)
        .execute(&mut **tx)
        .await
        .map_err(backend("counting match pairs"))?;
    }
    Ok(())
}

async fn insert(tx: &mut Transaction<'_, Sqlite>, pairs: &[MatchPair]) -> Result<(), DbError> {
    for chunk in pairs.chunks(INSERT_CHUNK) {
        let values = vec!["(?, ?, ?, ?, ?)"; chunk.len()].join(", ");
        let sql = format!("INSERT INTO {MATCH_PAIRS_TABLE} (kind, a, b, band, score) VALUES {values}");
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
    let sql = list_query(placeholder, limit);
    let mut listed = Vec::new();
    for kind in kinds {
        let rows = sqlx::query(&sql)
            .bind(band_rank(min_band))
            .bind(kind.as_str())
            .fetch_all(pool)
            .await
            .map_err(backend("reading the match pairs"))?;
        for row in rows {
            listed.push(MatchPair {
                kind: kind_of(row.get("kind"))?,
                a: row.get("a"),
                b: row.get("b"),
                band: band_of(row.get("band"))?,
                score: row.get("score"),
            });
        }
    }
    Ok(strongest(listed, limit))
}

/// How many undecided pairs from `min_band` up each kind holds, for the kinds holding any, in kind
/// name order.
///
/// # Errors
///
/// A [`DbError`] on a read failure.
pub(crate) async fn counts(pool: &Pool<Sqlite>, min_band: MatchBand) -> Result<Vec<(MatchableKind, usize)>, DbError> {
    let stored = per_kind(pool, &stored_count_query(placeholder), min_band).await?;
    let decided = per_kind(pool, &decided_query(placeholder), min_band).await?;
    undecided_counts(stored, &decided)
}

/// The `(kind, n)` rows a count query from band `min_band` up yields.
async fn per_kind(pool: &Pool<Sqlite>, sql: &str, min_band: MatchBand) -> Result<Vec<(MatchableKind, i64)>, DbError> {
    let rows = sqlx::query(sql)
        .bind(band_rank(min_band))
        .fetch_all(pool)
        .await
        .map_err(backend("counting the match pairs"))?;
    let mut counts = Vec::with_capacity(rows.len());
    for row in rows {
        counts.push((kind_of(row.get("kind"))?, row.get::<i64, _>("n")));
    }
    Ok(counts)
}
