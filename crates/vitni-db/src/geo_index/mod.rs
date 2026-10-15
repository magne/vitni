//! The Place geometry spatial index (ADR 0024 §3): WKB plus a bounding-box index over every live
//! place geometry, so [`Store::places_in_bbox`](crate::Store::places_in_bbox) answers a viewport query
//! without scanning every place.
//!
//! Derived and rebuildable from the event log (ADR 0010): every row is recomputed from the
//! already-updated Place projection, never asserted directly. `place_geometry` holds one row per live
//! geometry — its GeoPackage-encoded WKB (`geozero`) and its exact bounding box in degrees. Each
//! engine indexes that box its own way: SQLite with the R\*Tree virtual table it ships (no extension),
//! Postgres with a `GiST` index over its built-in `box` type (no `PostGIS`).
//!
//! **Both engines must give the same answer, so neither index is trusted on its own.** SQLite's
//! R\*Tree stores 32-bit floats rounded outward, and Postgres compares `box`es with a 1e-6 tolerance, so
//! either would report a place one microdegree outside the query. Each engine uses its index only to
//! narrow the candidates, then checks them exactly against the stored `f64` bounds: a geometry matches
//! when its box overlaps the query's, edges and corners included. Both engines store the same
//! [`IndexRow`] values and apply the same inclusive comparison, so they agree by construction —
//! `tests/spatial.rs` holds them to one corpus.
//!
//! `PostGIS` was not used: its `&&` compares 32-bit boxes too, so it would need the same exact check,
//! and it would require `CREATE EXTENSION` (a superuser step) on every Postgres workspace.
//!
//! One engine-neutral half (the rows each geometry yields) and one thin submodule per backend,
//! mirroring the `place_succession_index` split.

#[cfg(feature = "postgres")]
pub(crate) mod postgres;
#[cfg(feature = "sqlite")]
pub(crate) mod sqlite;

use geo_types::{Coord, Geometry, LineString, Point, Polygon};
use geozero::{CoordDimensions, ToWkb};
use vitni_core::geo::{GeoCoordinates, PlaceGeometry};
use vitni_core::place::PlaceView;

use crate::store::DbError;

/// The place-geometry table: one row per live geometry, keyed by its own `id`.
const PLACE_GEOMETRY_TABLE: &str = "place_geometry";

/// One geometry's row: its `GeoPackage` WKB and its exact bounding box in degrees.
struct IndexRow {
    wkb: Vec<u8>,
    min_lon: f64,
    max_lon: f64,
    min_lat: f64,
    max_lat: f64,
}

/// Every index row a place currently yields: one per geometry it carries (the legacy undated
/// `coordinates` point, plus every dated `geometries` assertion — ADR 0024).
///
/// # Errors
///
/// A [`DbError`] if a geometry cannot be encoded as WKB.
fn index_rows(view: &PlaceView) -> Result<Vec<IndexRow>, DbError> {
    let mut rows = Vec::new();
    for geometry in place_geometries(view) {
        if let Some(row) = index_row(&geometry)? {
            rows.push(row);
        }
    }
    Ok(rows)
}

/// Encodes `geometry` as `GeoPackage` WKB alongside its bounding box; `None` for an empty polygon,
/// which `decide` already rejects before an event exists — defensive, not reachable in practice.
fn index_row(geometry: &PlaceGeometry) -> Result<Option<IndexRow>, DbError> {
    let Some((min_lat, min_lon, max_lat, max_lon)) = geometry.bounding_box() else {
        return Ok(None);
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
    Ok(Some(IndexRow {
        wkb,
        min_lon,
        max_lon,
        min_lat,
        max_lat,
    }))
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
