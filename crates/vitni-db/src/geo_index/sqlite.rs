//! The SQLite half of the Place geometry spatial index (ADR 0024 §3) — see the
//! [module header](super) for what the table holds and why each match is checked exactly.
//!
//! `place_geometry_rtree` is the R\*Tree virtual table SQLite ships with, joined to `place_geometry`
//! by `id`; its 32-bit, outward-rounded boxes only narrow the candidates.

use async_trait::async_trait;
use cqrs_es::{EventEnvelope, Query};
use sqlx::{Pool, Row, Sqlite};
use vitni_core::place::{PlaceState, PlaceView};

use super::{IndexRow, PLACE_GEOMETRY_TABLE, index_rows};
use crate::sqlite_query;
use crate::store::DbError;
use crate::tables::PLACE_VIEW_TABLE;

/// The SQLite R\*Tree virtual table indexing each row's bounding box, joined to
/// [`PLACE_GEOMETRY_TABLE`] by `id`.
const PLACE_GEOMETRY_RTREE_TABLE: &str = "place_geometry_rtree";

const CREATE_PLACE_GEOMETRY_TABLE: &str = "
CREATE TABLE IF NOT EXISTS place_geometry (
    id       INTEGER PRIMARY KEY,
    place_id TEXT    NOT NULL,
    wkb      BLOB    NOT NULL,
    min_lon  REAL    NOT NULL,
    max_lon  REAL    NOT NULL,
    min_lat  REAL    NOT NULL,
    max_lat  REAL    NOT NULL
)";

const CREATE_PLACE_GEOMETRY_RTREE: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS place_geometry_rtree USING rtree(
    id,
    min_lon, max_lon,
    min_lat, max_lat
)";

/// Creates the geometry-index tables on a fresh (or existing) workspace database, returning whether
/// they are new and so must be filled from the Place projection once it is open. A table from before
/// the exact bounding-box columns is dropped and recreated (it is derived, so nothing is lost).
///
/// # Errors
///
/// Returns the `sqlx` error if a schema query fails.
pub(crate) async fn create_tables(pool: &Pool<Sqlite>) -> Result<bool, sqlx::Error> {
    let columns = sqlx::query(&format!("PRAGMA table_xinfo({PLACE_GEOMETRY_TABLE})"))
        .fetch_all(pool)
        .await?;
    let current = columns.iter().any(|row| row.get::<String, _>("name") == "min_lat");
    if !columns.is_empty() && !current {
        sqlx::query(&format!("DROP TABLE IF EXISTS {PLACE_GEOMETRY_RTREE_TABLE}"))
            .execute(pool)
            .await?;
        sqlx::query(&format!("DROP TABLE {PLACE_GEOMETRY_TABLE}"))
            .execute(pool)
            .await?;
    }
    sqlx::query(CREATE_PLACE_GEOMETRY_TABLE).execute(pool).await?;
    sqlx::query(CREATE_PLACE_GEOMETRY_RTREE).execute(pool).await?;
    Ok(!current)
}

/// Deletes every row from both geometry-index tables — the rebuild's clearing step (ADR 0010).
async fn clear_tables(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(&format!("DELETE FROM {PLACE_GEOMETRY_RTREE_TABLE}"))
        .execute(pool)
        .await?;
    sqlx::query(&format!("DELETE FROM {PLACE_GEOMETRY_TABLE}"))
        .execute(pool)
        .await?;
    Ok(())
}

/// A `cqrs-es` query that keeps the geometry index in step with the Place projection: on every
/// committed batch of Place events, recomputes that place's rows from its current projection. Must
/// be appended *after* the place projection query in the framework's query list, so the projection it
/// reads is already up to date.
pub(crate) struct PlaceGeometryIndexQuery {
    pool: Pool<Sqlite>,
}

impl PlaceGeometryIndexQuery {
    /// Wraps the pool the projection and geometry-index tables share.
    pub(crate) fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Query<PlaceState> for PlaceGeometryIndexQuery {
    async fn dispatch(&self, aggregate_id: &str, _events: &[EventEnvelope<PlaceState>]) {
        if let Err(error) = reindex_place(&self.pool, aggregate_id).await {
            tracing::error!(place_id = aggregate_id, %error, "failed to update the place geometry index");
        }
    }
}

/// Recomputes one place's geometry-index rows from its current projection: clears its prior rows,
/// then inserts one row per live geometry.
async fn reindex_place(pool: &Pool<Sqlite>, place_id: &str) -> Result<(), DbError> {
    delete_place_rows(pool, place_id).await?;
    let Some(view) = sqlite_query::find_view_by_id::<PlaceView>(pool, PLACE_VIEW_TABLE, place_id).await? else {
        return Ok(());
    };
    for row in index_rows(&view)? {
        insert_row(pool, place_id, &row).await?;
    }
    Ok(())
}

/// Rebuilds the whole geometry index from every place's (already-rebuilt) projection — the
/// maintenance path `Store::rebuild_projections` drives (ADR 0010). Reuses [`insert_row`], so a
/// rebuilt index is byte-identical to the live-dispatch path.
///
/// # Errors
///
/// A [`DbError`] if clearing the tables or reading/writing a projection fails.
pub(crate) async fn rebuild_index(pool: &Pool<Sqlite>) -> Result<(), DbError> {
    clear_tables(pool)
        .await
        .map_err(|e| DbError::Backend(format!("clearing place geometry index: {e}")))?;
    let views: Vec<PlaceView> = sqlite_query::list_views(pool, PLACE_VIEW_TABLE).await?;
    for view in &views {
        let Some(place_id) = view.place_id() else { continue };
        let place_id = place_id.to_string();
        for row in index_rows(view)? {
            insert_row(pool, &place_id, &row).await?;
        }
    }
    Ok(())
}

/// Answers a viewport query: every place with at least one geometry whose bounding box overlaps the
/// given bounding box, edges included, ordered by place id. The R\*Tree narrows the candidates; the
/// exact `f64` bounds decide (see the [module header](super)).
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn places_in_bbox(
    pool: &Pool<Sqlite>,
    min_lat: f64,
    min_lon: f64,
    max_lat: f64,
    max_lon: f64,
) -> Result<Vec<String>, DbError> {
    let sql = format!(
        "SELECT DISTINCT g.place_id FROM {PLACE_GEOMETRY_RTREE_TABLE} r \
         JOIN {PLACE_GEOMETRY_TABLE} g ON g.id = r.id \
         WHERE r.min_lon <= ?1 AND r.max_lon >= ?2 AND r.min_lat <= ?3 AND r.max_lat >= ?4 \
         AND g.min_lon <= ?1 AND g.max_lon >= ?2 AND g.min_lat <= ?3 AND g.max_lat >= ?4 \
         ORDER BY g.place_id"
    );
    let rows = sqlx::query(&sql)
        .bind(max_lon)
        .bind(min_lon)
        .bind(max_lat)
        .bind(min_lat)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(rows.iter().map(|row| row.get::<String, _>("place_id")).collect())
}

/// Deletes one place's prior geometry-index rows (both tables), so a reindex never leaves a stale
/// or retracted geometry behind.
async fn delete_place_rows(pool: &Pool<Sqlite>, place_id: &str) -> Result<(), DbError> {
    let sql = format!("SELECT id FROM {PLACE_GEOMETRY_TABLE} WHERE place_id = ?");
    let ids: Vec<i64> = sqlx::query(&sql)
        .bind(place_id)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?
        .iter()
        .map(|row| row.get::<i64, _>("id"))
        .collect();
    for id in ids {
        sqlx::query(&format!("DELETE FROM {PLACE_GEOMETRY_RTREE_TABLE} WHERE id = ?"))
            .bind(id)
            .execute(pool)
            .await
            .map_err(|e| DbError::Backend(e.to_string()))?;
    }
    sqlx::query(&format!("DELETE FROM {PLACE_GEOMETRY_TABLE} WHERE place_id = ?"))
        .bind(place_id)
        .execute(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(())
}

/// Inserts one geometry's row into each geometry-index table.
async fn insert_row(pool: &Pool<Sqlite>, place_id: &str, row: &IndexRow) -> Result<(), DbError> {
    let sql = format!(
        "INSERT INTO {PLACE_GEOMETRY_TABLE} (place_id, wkb, min_lon, max_lon, min_lat, max_lat) \
         VALUES (?, ?, ?, ?, ?, ?)"
    );
    let inserted = sqlx::query(&sql)
        .bind(place_id)
        .bind(&row.wkb)
        .bind(row.min_lon)
        .bind(row.max_lon)
        .bind(row.min_lat)
        .bind(row.max_lat)
        .execute(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    let row_id = inserted.last_insert_rowid();

    let sql = format!(
        "INSERT INTO {PLACE_GEOMETRY_RTREE_TABLE} (id, min_lon, max_lon, min_lat, max_lat) VALUES (?, ?, ?, ?, ?)"
    );
    sqlx::query(&sql)
        .bind(row_id)
        .bind(row.min_lon)
        .bind(row.max_lon)
        .bind(row.min_lat)
        .bind(row.max_lat)
        .execute(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(())
}
