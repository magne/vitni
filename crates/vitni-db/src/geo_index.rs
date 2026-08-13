//! WKB + SQLite R\*Tree spatial index over Place geometry (ADR 0024 §3), sqlite backend only.
//!
//! Derived and rebuildable from the event log (ADR 0010): every row here is recomputed from the
//! already-updated Place projection, never asserted directly. `place_geometry` stores each live
//! geometry as a GeoPackage-encoded WKB blob (`geozero`); `place_geometry_rtree` is the R\*Tree
//! virtual table SQLite ships with (no extension needed), indexing each blob's bounding box, so
//! [`places_in_bbox`] answers a viewport query without scanning every place. The Postgres mirror
//! (native geometry + `GiST`) is a follow-up (ADR 0024 §3) — this module is sqlite-only.

use async_trait::async_trait;
use cqrs_es::{EventEnvelope, Query};
use geo_types::{Coord, Geometry, LineString, Point, Polygon};
use geozero::{CoordDimensions, ToWkb};
use sqlx::{Pool, Row, Sqlite};
use vitni_core::geo::{GeoCoordinates, PlaceGeometry};
use vitni_core::place::{PlaceState, PlaceView};

use crate::sqlite_query;
use crate::store::DbError;
use crate::tables::PLACE_VIEW_TABLE;

/// The place-geometry blob table: one row per live geometry, keyed by its own `id` (ADR 0024 §3).
const PLACE_GEOMETRY_TABLE: &str = "place_geometry";
/// The SQLite R\*Tree virtual table indexing each row's bounding box, joined to
/// [`PLACE_GEOMETRY_TABLE`] by `id`.
const PLACE_GEOMETRY_RTREE_TABLE: &str = "place_geometry_rtree";

const CREATE_PLACE_GEOMETRY_TABLE: &str = "
CREATE TABLE IF NOT EXISTS place_geometry (
    id       INTEGER PRIMARY KEY,
    place_id TEXT    NOT NULL,
    wkb      BLOB    NOT NULL
)";

const CREATE_PLACE_GEOMETRY_RTREE: &str = "
CREATE VIRTUAL TABLE IF NOT EXISTS place_geometry_rtree USING rtree(
    id,
    min_lon, max_lon,
    min_lat, max_lat
)";

/// Creates the geometry-index tables on a fresh (or existing) workspace database. Idempotent.
///
/// # Errors
///
/// Returns the `sqlx` error if a `CREATE TABLE` statement fails.
pub(crate) async fn create_tables(pool: &Pool<Sqlite>) -> Result<(), sqlx::Error> {
    sqlx::query(CREATE_PLACE_GEOMETRY_TABLE).execute(pool).await?;
    sqlx::query(CREATE_PLACE_GEOMETRY_RTREE).execute(pool).await?;
    Ok(())
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
/// be appended *after* the place `GenericQuery` in the framework's query list, so the projection it
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
/// then inserts one WKB + bounding-box row per live geometry (the legacy undated `coordinates`
/// point, plus every dated `geometries` assertion — ADR 0024).
async fn reindex_place(pool: &Pool<Sqlite>, place_id: &str) -> Result<(), DbError> {
    delete_place_rows(pool, place_id).await?;
    let Some(view) = sqlite_query::find_view_by_id::<PlaceView>(pool, PLACE_VIEW_TABLE, place_id).await? else {
        return Ok(());
    };
    for geometry in place_geometries(&view) {
        insert_row(pool, place_id, &geometry).await?;
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
        for geometry in place_geometries(view) {
            insert_row(pool, &place_id, &geometry).await?;
        }
    }
    Ok(())
}

/// Answers a viewport query: every place with at least one geometry whose bounding box overlaps
/// the given bounding box (ADR 0024 §3), without scanning every place.
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
         WHERE r.min_lon <= ? AND r.max_lon >= ? AND r.min_lat <= ? AND r.max_lat >= ?"
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

/// Encodes `geometry` as `GeoPackage` WKB and inserts one row into each geometry-index table.
async fn insert_row(pool: &Pool<Sqlite>, place_id: &str, geometry: &PlaceGeometry) -> Result<(), DbError> {
    let Some((min_lat, min_lon, max_lat, max_lon)) = geometry.bounding_box() else {
        // An empty polygon; `decide` already rejects this before an event exists, so a live
        // projection never reaches here. Defensive, not reachable in practice.
        return Ok(());
    };
    let (min_lon, max_lon, min_lat, max_lat) = (
        min_lon.to_degrees(),
        max_lon.to_degrees(),
        min_lat.to_degrees(),
        max_lat.to_degrees(),
    );

    let wkb = to_geo_types(geometry)
        .to_gpkg_wkb(
            CoordDimensions::xy(),
            Some(4326),
            vec![min_lon, max_lon, min_lat, max_lat],
        )
        .map_err(|e| DbError::Backend(format!("encoding place geometry WKB: {e}")))?;

    let sql = format!("INSERT INTO {PLACE_GEOMETRY_TABLE} (place_id, wkb) VALUES (?, ?)");
    let inserted = sqlx::query(&sql)
        .bind(place_id)
        .bind(&wkb)
        .execute(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    let row_id = inserted.last_insert_rowid();

    let sql = format!(
        "INSERT INTO {PLACE_GEOMETRY_RTREE_TABLE} (id, min_lon, max_lon, min_lat, max_lat) VALUES (?, ?, ?, ?, ?)"
    );
    sqlx::query(&sql)
        .bind(row_id)
        .bind(min_lon)
        .bind(max_lon)
        .bind(min_lat)
        .bind(max_lat)
        .execute(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(())
}

/// Every geometry a place currently carries: the legacy undated `coordinates` point (ADR 0024's
/// "`CoordinatesAsserted` is the undated `Point` case"), plus every dated `geometries` assertion.
fn place_geometries(view: &PlaceView) -> Vec<PlaceGeometry> {
    let mut geometries = Vec::new();
    if let Some(coordinates) = view.coordinates() {
        geometries.push(PlaceGeometry::Point(*coordinates));
    }
    geometries.extend(
        view.geometries()
            .into_iter()
            .map(|assertion| assertion.geometry.clone()),
    );
    geometries
}

/// Converts the pure, integer-`Microdegrees` [`PlaceGeometry`] to a floating-point `geo-types`
/// geometry, the boundary conversion `geozero`'s WKB writer needs.
fn to_geo_types(geometry: &PlaceGeometry) -> Geometry<f64> {
    match geometry {
        PlaceGeometry::Point(point) => {
            Geometry::Point(Point::new(point.longitude.to_degrees(), point.latitude.to_degrees()))
        }
        PlaceGeometry::Polygon { exterior, holes } => {
            let exterior = ring(exterior);
            let holes = holes.iter().map(|hole| ring(hole)).collect();
            Geometry::Polygon(Polygon::new(exterior, holes))
        }
    }
}

/// Converts one ring of [`GeoCoordinates`] to a `geo-types` `LineString`.
fn ring(points: &[GeoCoordinates]) -> LineString<f64> {
    points
        .iter()
        .map(|point| Coord {
            x: point.longitude.to_degrees(),
            y: point.latitude.to_degrees(),
        })
        .collect()
}
