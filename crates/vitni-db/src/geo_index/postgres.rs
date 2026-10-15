//! The Postgres half of the Place geometry spatial index (ADR 0024 §3) — see the
//! [module header](super) for what the table holds and why each match is checked exactly.
//!
//! A function-for-function twin of [`sqlite`](super::sqlite). Where it differs, on purpose:
//!
//! - **One table, not two.** The bounding box is indexed by a `GiST` expression index over Postgres's
//!   built-in `box` type, built from the same exact columns SQLite's R\*Tree mirrors; `box &&`
//!   compares with a 1e-6 tolerance, so it only narrows the candidates.
//! - **`id` is an identity column and [`clear_tables`] is `DELETE FROM`**, as in the succession
//!   index: no `ACCESS EXCLUSIVE` lock, and nothing reads `id` beyond joining rows.
//! - **A `place_id` index** the SQLite table lacks, so a reindex does not seq-scan a table that
//!   grows with the whole workspace. Results are unaffected.
//!
//! Like both halves of the succession index, [`reindex_place`] is delete-then-insert with no
//! enclosing transaction; the index is derived and `rebuild_index` repairs it.

use async_trait::async_trait;
use cqrs_es::{EventEnvelope, Query};
use sqlx::{Pool, Postgres, Row};
use vitni_core::place::{PlaceState, PlaceView};

use super::{IndexRow, PLACE_GEOMETRY_TABLE, index_rows};
use crate::postgres_query;
use crate::store::DbError;
use crate::tables::PLACE_VIEW_TABLE;

const CREATE_PLACE_GEOMETRY_TABLE: &str = "
CREATE TABLE IF NOT EXISTS place_geometry (
    id       BIGINT GENERATED ALWAYS AS IDENTITY PRIMARY KEY,
    place_id TEXT             NOT NULL,
    wkb      BYTEA            NOT NULL,
    min_lon  DOUBLE PRECISION NOT NULL,
    max_lon  DOUBLE PRECISION NOT NULL,
    min_lat  DOUBLE PRECISION NOT NULL,
    max_lat  DOUBLE PRECISION NOT NULL
)";

/// The bounding box as a `box`, the expression the `GiST` index covers and the query must repeat
/// verbatim for the planner to use it.
const BOUNDING_BOX: &str = "box(point(min_lon, min_lat), point(max_lon, max_lat))";

/// Creates the geometry-index table and its indexes on a fresh (or existing) workspace database,
/// returning whether the table is new and so must be filled from the Place projection once it is
/// open.
///
/// # Errors
///
/// Returns the `sqlx` error if a schema query fails.
pub(crate) async fn create_tables(pool: &Pool<Postgres>) -> Result<bool, sqlx::Error> {
    let existed: bool = sqlx::query("SELECT to_regclass($1) IS NOT NULL AS existed")
        .bind(PLACE_GEOMETRY_TABLE)
        .fetch_one(pool)
        .await?
        .get("existed");
    sqlx::query(CREATE_PLACE_GEOMETRY_TABLE).execute(pool).await?;
    sqlx::query(&format!(
        "CREATE INDEX IF NOT EXISTS place_geometry_place_idx ON {PLACE_GEOMETRY_TABLE} (place_id)"
    ))
    .execute(pool)
    .await?;
    sqlx::query(&format!(
        "CREATE INDEX IF NOT EXISTS place_geometry_bbox_idx ON {PLACE_GEOMETRY_TABLE} USING gist (({BOUNDING_BOX}))"
    ))
    .execute(pool)
    .await?;
    Ok(!existed)
}

/// Deletes every row from the geometry-index table — the rebuild's clearing step (ADR 0010).
async fn clear_tables(pool: &Pool<Postgres>) -> Result<(), sqlx::Error> {
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
    pool: Pool<Postgres>,
}

impl PlaceGeometryIndexQuery {
    /// Wraps the pool the projection and geometry-index tables share.
    pub(crate) fn new(pool: Pool<Postgres>) -> Self {
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
async fn reindex_place(pool: &Pool<Postgres>, place_id: &str) -> Result<(), DbError> {
    sqlx::query(&format!("DELETE FROM {PLACE_GEOMETRY_TABLE} WHERE place_id = $1"))
        .bind(place_id)
        .execute(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    let Some(view) = postgres_query::find_view_by_id::<PlaceView>(pool, PLACE_VIEW_TABLE, place_id).await? else {
        return Ok(());
    };
    for row in index_rows(&view)? {
        insert_row(pool, place_id, &row).await?;
    }
    Ok(())
}

/// Rebuilds the whole geometry index from every place's (already-rebuilt) projection — the
/// maintenance path `Store::rebuild_projections` drives (ADR 0010). Reuses [`insert_row`], so a
/// rebuilt index holds the same rows as the live-dispatch path.
///
/// # Errors
///
/// A [`DbError`] if clearing the table or reading/writing a projection fails.
pub(crate) async fn rebuild_index(pool: &Pool<Postgres>) -> Result<(), DbError> {
    clear_tables(pool)
        .await
        .map_err(|e| DbError::Backend(format!("clearing place geometry index: {e}")))?;
    let views: Vec<PlaceView> = postgres_query::list_views(pool, PLACE_VIEW_TABLE).await?;
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
/// given bounding box, edges included, ordered by place id. The `GiST` index narrows the candidates;
/// the exact `f64` bounds decide (see the [module header](super)).
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn places_in_bbox(
    pool: &Pool<Postgres>,
    min_lat: f64,
    min_lon: f64,
    max_lat: f64,
    max_lon: f64,
) -> Result<Vec<String>, DbError> {
    let sql = format!(
        "SELECT DISTINCT place_id COLLATE \"C\" AS place_id FROM {PLACE_GEOMETRY_TABLE} \
         WHERE {BOUNDING_BOX} && box(point($2, $4), point($1, $3)) \
         AND min_lon <= $1 AND max_lon >= $2 AND min_lat <= $3 AND max_lat >= $4 \
         ORDER BY 1"
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

/// Inserts one geometry's row.
async fn insert_row(pool: &Pool<Postgres>, place_id: &str, row: &IndexRow) -> Result<(), DbError> {
    let sql = format!(
        "INSERT INTO {PLACE_GEOMETRY_TABLE} (place_id, wkb, min_lon, max_lon, min_lat, max_lat) \
         VALUES ($1, $2, $3, $4, $5, $6)"
    );
    sqlx::query(&sql)
        .bind(place_id)
        .bind(&row.wkb)
        .bind(row.min_lon)
        .bind(row.max_lon)
        .bind(row.min_lat)
        .bind(row.max_lat)
        .execute(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(())
}
