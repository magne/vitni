//! The SQLite half of the Place succession cross-reference index (ADR 0026 §4) — see the
//! [module header](super) for what the two tables hold and why the index exists.

use async_trait::async_trait;
use cqrs_es::{EventEnvelope, Query};
use sqlx::{Pool, Row, Sqlite};
use vitni_core::place::{PlaceState, PlaceView};

use super::{PLACE_SUCCESSION_LINK_TABLE, PLACE_SUCCESSION_TABLE, SuccessionAssertion, succession_columns};
use crate::sqlite_query;
use crate::store::{DbError, PlaceSuccessionRecord};
use crate::tables::PLACE_VIEW_TABLE;

const CREATE_PLACE_SUCCESSION_TABLE: &str = "
CREATE TABLE IF NOT EXISTS place_succession (
    id               INTEGER PRIMARY KEY,
    anchor_place_id  TEXT    NOT NULL,
    assertion_id     TEXT    NOT NULL,
    kind             TEXT    NOT NULL,
    date_json        TEXT
)";

const CREATE_PLACE_SUCCESSION_LINK_TABLE: &str = "
CREATE TABLE IF NOT EXISTS place_succession_link (
    succession_id   INTEGER NOT NULL,
    from_place_id   TEXT    NOT NULL,
    to_place_id     TEXT    NOT NULL
)";

/// Creates the succession-index tables on a fresh (or existing) workspace database. Idempotent.
///
/// # Errors
///
/// Returns the `sqlx` error if a `CREATE TABLE` statement fails.
pub(crate) async fn create_tables(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(CREATE_PLACE_SUCCESSION_TABLE).execute(pool).await?;
    sqlx::query(CREATE_PLACE_SUCCESSION_LINK_TABLE).execute(pool).await?;
    Ok(())
}

/// Deletes every row from both succession-index tables — the rebuild's clearing step (ADR 0010).
async fn clear_tables(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DELETE FROM {PLACE_SUCCESSION_LINK_TABLE}"))
        .execute(pool)
        .await?;
    sqlx::query(&format!("DELETE FROM {PLACE_SUCCESSION_TABLE}"))
        .execute(pool)
        .await?;
    Ok(())
}

/// A `cqrs-es` query that keeps the succession index in step with the Place projection: on every
/// committed batch of Place events, recomputes that place's (anchor) rows from its current
/// projection. Must be appended *after* the place `GenericQuery`, so the projection it reads is
/// already up to date.
pub(crate) struct PlaceSuccessionIndexQuery {
    pool: Pool<Sqlite>,
}

impl PlaceSuccessionIndexQuery {
    /// Wraps the pool the projection and succession-index tables share.
    pub(crate) fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Query<PlaceState> for PlaceSuccessionIndexQuery {
    async fn dispatch(&self, aggregate_id: &str, _events: &[EventEnvelope<PlaceState>]) {
        if let Err(error) = reindex_place(&self.pool, aggregate_id).await {
            tracing::error!(place_id = aggregate_id, %error, "failed to update the place succession index");
        }
    }
}

/// Recomputes one (anchor) place's succession-index rows from its current projection: clears its
/// prior rows, then inserts one metadata row plus one link row per `(from, to)` pair for each live
/// succession assertion it carries.
async fn reindex_place(pool: &Pool<Sqlite>, anchor_place_id: &str) -> Result<(), DbError> {
    delete_anchor_rows(pool, anchor_place_id).await?;
    let Some(view) = sqlite_query::find_view_by_id::<PlaceView>(pool, PLACE_VIEW_TABLE, anchor_place_id).await? else {
        return Ok(());
    };
    for attributed in view.successions_with_assertions() {
        insert_succession(pool, anchor_place_id, attributed).await?;
    }
    Ok(())
}

/// Rebuilds the whole succession index from every place's (already-rebuilt) projection — the
/// maintenance path `Store::rebuild_projections` drives (ADR 0010). Reuses [`insert_succession`], so
/// a rebuilt index is byte-identical to the live-dispatch path.
///
/// # Errors
///
/// A [`DbError`] if clearing the tables or reading/writing a projection fails.
pub(crate) async fn rebuild_index(pool: &Pool<Sqlite>) -> Result<(), DbError> {
    clear_tables(pool)
        .await
        .map_err(|e| DbError::Backend(format!("clearing place succession index: {e}")))?;
    let views: Vec<PlaceView> = sqlite_query::list_views(pool, PLACE_VIEW_TABLE).await?;
    for view in &views {
        let Some(place_id) = view.place_id() else { continue };
        let place_id = place_id.to_string();
        for attributed in view.successions_with_assertions() {
            insert_succession(pool, &place_id, attributed).await?;
        }
    }
    Ok(())
}

/// Every place a succession names `to`, from `place_id`'s perspective as a `from` endpoint (what did
/// this place become?), newest-assertion-first.
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn successors(pool: &Pool<Sqlite>, place_id: &str) -> Result<Vec<PlaceSuccessionRecord>, DbError> {
    query_side(pool, place_id, "from_place_id", "to_place_id").await
}

/// Every place a succession names `from`, from `place_id`'s perspective as a `to` endpoint (what did
/// this place come from?), newest-assertion-first.
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn predecessors(pool: &Pool<Sqlite>, place_id: &str) -> Result<Vec<PlaceSuccessionRecord>, DbError> {
    query_side(pool, place_id, "to_place_id", "from_place_id").await
}

/// Shared implementation for [`successors`]/[`predecessors`]: joins the link table (matched on
/// `match_column`) to the metadata table, returning the counterpart id from `select_column`.
async fn query_side(
    pool: &Pool<Sqlite>,
    place_id: &str,
    match_column: &str,
    select_column: &str,
) -> Result<Vec<PlaceSuccessionRecord>, DbError> {
    let sql = format!(
        "SELECT l.{select_column} AS counterpart, s.kind, s.date_json, s.assertion_id \
         FROM {PLACE_SUCCESSION_LINK_TABLE} l \
         JOIN {PLACE_SUCCESSION_TABLE} s ON s.id = l.succession_id \
         WHERE l.{match_column} = ? \
         ORDER BY s.id DESC"
    );
    let rows = sqlx::query(&sql)
        .bind(place_id)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(rows
        .iter()
        .map(|row| PlaceSuccessionRecord {
            place_id: row.get::<String, _>("counterpart"),
            kind: row.get::<String, _>("kind"),
            date_json: row.get::<Option<String>, _>("date_json"),
            assertion_id: row.get::<String, _>("assertion_id"),
        })
        .collect())
}

/// Deletes one anchor place's prior succession-index rows (both tables), so a reindex never leaves a
/// stale or retracted succession behind.
async fn delete_anchor_rows(pool: &Pool<Sqlite>, anchor_place_id: &str) -> Result<(), DbError> {
    let sql = format!("SELECT id FROM {PLACE_SUCCESSION_TABLE} WHERE anchor_place_id = ?");
    let ids: Vec<i64> = sqlx::query(&sql)
        .bind(anchor_place_id)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?
        .iter()
        .map(|row| row.get::<i64, _>("id"))
        .collect();
    for id in ids {
        sqlx::query(&format!(
            "DELETE FROM {PLACE_SUCCESSION_LINK_TABLE} WHERE succession_id = ?"
        ))
        .bind(id)
        .execute(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    }
    sqlx::query(&format!(
        "DELETE FROM {PLACE_SUCCESSION_TABLE} WHERE anchor_place_id = ?"
    ))
    .bind(anchor_place_id)
    .execute(pool)
    .await
    .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(())
}

/// Inserts one metadata row plus one link row per `(from, to)` pair (the cartesian product of the
/// assertion's endpoint lists) for a single live succession assertion.
async fn insert_succession(
    pool: &Pool<Sqlite>,
    anchor_place_id: &str,
    attributed: &SuccessionAssertion,
) -> Result<(), DbError> {
    let assertion = &attributed.value.value;
    let (kind, date_json) = succession_columns(attributed)?;

    let sql = format!(
        "INSERT INTO {PLACE_SUCCESSION_TABLE} (anchor_place_id, assertion_id, kind, date_json) VALUES (?, ?, ?, ?)"
    );
    let inserted = sqlx::query(&sql)
        .bind(anchor_place_id)
        .bind(attributed.assertion_id.to_string())
        .bind(&kind)
        .bind(&date_json)
        .execute(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    let succession_id = inserted.last_insert_rowid();

    for from in &assertion.from {
        for to in &assertion.to {
            let sql = format!(
                "INSERT INTO {PLACE_SUCCESSION_LINK_TABLE} (succession_id, from_place_id, to_place_id) \
                 VALUES (?, ?, ?)"
            );
            sqlx::query(&sql)
                .bind(succession_id)
                .bind(from.to_string())
                .bind(to.to_string())
                .execute(pool)
                .await
                .map_err(|e| DbError::Backend(e.to_string()))?;
        }
    }
    Ok(())
}
