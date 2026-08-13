//! Geographic coordinates for a [`Place`](crate::place) (data-model §7).
//!
//! Latitude and longitude are stored as **microdegrees** (millionths of a degree) — a scaled `i32`
//! — so the value object keeps `Eq` and a byte-stable serialization (see [`crate::fixed`]).
//! ±1e-6° is roughly 0.1 m, finer than any genealogical source needs.

use std::str::FromStr;

use serde::{Deserialize, Serialize};

use crate::fixed::{ParseFixedError, fixed_decimal_display, parse_decimal};

/// A degree value stored as microdegrees (millionths of a degree — data-model §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct Microdegrees(i32);

impl Microdegrees {
    /// Millionths of a degree.
    const SCALE: u32 = 6;

    /// Wraps a count of microdegrees.
    #[must_use]
    pub const fn from_microdegrees(microdegrees: i32) -> Self {
        Self(microdegrees)
    }

    /// Returns the value in microdegrees.
    #[must_use]
    pub const fn as_microdegrees(self) -> i32 {
        self.0
    }

    /// Returns the value as a floating-point degree count, for boundary conversions (WKB, `GeoJSON` —
    /// ADR 0024) that need `f64`. The event log and every in-process comparison stay on this integer
    /// representation; floats appear only at those encoding boundaries.
    #[must_use]
    pub fn to_degrees(self) -> f64 {
        f64::from(self.0) / f64::from(10_i32.pow(Self::SCALE))
    }
}

fixed_decimal_display!(Microdegrees, Microdegrees::SCALE);

impl FromStr for Microdegrees {
    type Err = ParseFixedError;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        let scaled = parse_decimal(value, Self::SCALE)?;
        i32::try_from(scaled)
            .map(Self)
            .map_err(|_| ParseFixedError::OutOfRange(value.to_owned()))
    }
}

/// A geographic point (data-model §7).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct GeoCoordinates {
    /// Latitude in degrees (north positive).
    pub latitude: Microdegrees,
    /// Longitude in degrees (east positive).
    pub longitude: Microdegrees,
}

/// A place's geographic shape (data-model §7, ADR 0024): a point, or a polygon (an exterior ring
/// plus optional holes), over the same integer [`Microdegrees`] coordinates as [`GeoCoordinates`] so
/// the value keeps `Eq` and a byte-stable serialization. `Point` subsumes the historical undated
/// coordinate assertion. `LineString` / `Multi*` variants are additive later (YAGNI — ADR 0024).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(tag = "kind")]
pub enum PlaceGeometry {
    /// A single point.
    Point(GeoCoordinates),
    /// A polygon: one exterior ring plus any interior holes.
    Polygon {
        /// The outer boundary.
        exterior: Vec<GeoCoordinates>,
        /// Interior holes cut out of the exterior, if any.
        #[serde(default)]
        holes: Vec<Vec<GeoCoordinates>>,
    },
}

impl PlaceGeometry {
    /// Every ring a `Polygon` carries (the exterior, then each hole) — empty for a `Point`. Used to
    /// validate ring length (data-model §10.1 `InvalidGeometry`) without exposing the shape's
    /// internals to the Place decision core.
    #[must_use]
    pub fn rings(&self) -> Vec<&Vec<GeoCoordinates>> {
        match self {
            Self::Point(_) => Vec::new(),
            Self::Polygon { exterior, holes } => std::iter::once(exterior).chain(holes.iter()).collect(),
        }
    }

    /// Every coordinate the shape carries, in no particular order.
    fn points(&self) -> Box<dyn Iterator<Item = GeoCoordinates> + '_> {
        match self {
            Self::Point(point) => Box::new(std::iter::once(*point)),
            Self::Polygon { exterior, holes } => Box::new(exterior.iter().chain(holes.iter().flatten()).copied()),
        }
    }

    /// The bounding box over every coordinate in the shape, as `(min_lat, min_lon, max_lat,
    /// max_lon)` — the input to a spatial index (ADR 0024 §3). `None` only for a `Polygon` with no
    /// points at all (an empty exterior and no holes), which the Place decision core already rejects
    /// before an event exists, so a live projection never sees it.
    #[must_use]
    pub fn bounding_box(&self) -> Option<(Microdegrees, Microdegrees, Microdegrees, Microdegrees)> {
        let mut points = self.points();
        let first = points.next()?;
        let mut min_lat = first.latitude;
        let mut max_lat = first.latitude;
        let mut min_lon = first.longitude;
        let mut max_lon = first.longitude;
        for point in points {
            min_lat = min_lat.min(point.latitude);
            max_lat = max_lat.max(point.latitude);
            min_lon = min_lon.min(point.longitude);
            max_lon = max_lon.max(point.longitude);
        }
        Some((min_lat, min_lon, max_lat, max_lon))
    }

    /// A single point standing in for the whole shape — the point itself, or (for a `Polygon`) the
    /// unweighted average of the exterior ring's vertices. Used to place an event pin at a place
    /// whose geometry is an area (ADR 0025 §1 "event-at-place pins") and by the geography view's
    /// marker placement; deliberately **not** a true area centroid (that would need floating-point
    /// polygon math this crate does not carry — `vitni-core` stays free of the `geo` algorithm
    /// crate, ADR 0024 §5), so it is a reasonable approximation for pin placement only, never a
    /// geometric authority. Interior holes do not pull the average (they carve out area, not add
    /// vertices to weight toward). `None` only for an empty `Polygon` (see [`Self::bounding_box`]).
    #[must_use]
    pub fn representative_point(&self) -> Option<GeoCoordinates> {
        match self {
            Self::Point(point) => Some(*point),
            Self::Polygon { exterior, .. } => {
                if exterior.is_empty() {
                    return None;
                }
                let count = i64::try_from(exterior.len()).unwrap_or(1).max(1);
                let (lat_sum, lon_sum) = exterior.iter().fold((0_i64, 0_i64), |(lat, lon), point| {
                    (
                        lat + i64::from(point.latitude.as_microdegrees()),
                        lon + i64::from(point.longitude.as_microdegrees()),
                    )
                });
                let average = |sum: i64| i32::try_from(sum / count).unwrap_or(i32::MAX);
                Some(GeoCoordinates {
                    latitude: Microdegrees::from_microdegrees(average(lat_sum)),
                    longitude: Microdegrees::from_microdegrees(average(lon_sum)),
                })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{GeoCoordinates, Microdegrees, PlaceGeometry};
    use std::str::FromStr;

    #[test]
    fn parses_and_renders_a_coordinate() {
        let latitude = Microdegrees::from_str("60.391262").unwrap();
        assert_eq!(latitude.as_microdegrees(), 60_391_262);
        assert_eq!(latitude.to_string(), "60.391262");
    }

    #[test]
    fn rejects_a_value_outside_i32() {
        assert!(Microdegrees::from_str("100000").is_err());
    }

    #[test]
    fn converts_to_floating_point_degrees() {
        let value = Microdegrees::from_str("60.391262").unwrap();
        assert!((value.to_degrees() - 60.391_262).abs() < f64::EPSILON);
    }

    #[test]
    fn coordinates_round_trip_through_json() {
        let point = GeoCoordinates {
            latitude: Microdegrees::from_str("60.39").unwrap(),
            longitude: Microdegrees::from_str("5.32").unwrap(),
        };
        let json = serde_json::to_string(&point).unwrap();
        let back: GeoCoordinates = serde_json::from_str(&json).unwrap();
        assert_eq!(point, back);
    }

    fn point(lat: &str, lon: &str) -> GeoCoordinates {
        GeoCoordinates {
            latitude: Microdegrees::from_str(lat).unwrap(),
            longitude: Microdegrees::from_str(lon).unwrap(),
        }
    }

    #[test]
    fn point_geometry_round_trips_through_json() {
        let geometry = PlaceGeometry::Point(point("60.39", "5.32"));
        let json = serde_json::to_string(&geometry).unwrap();
        let back: PlaceGeometry = serde_json::from_str(&json).unwrap();
        assert_eq!(geometry, back);
    }

    #[test]
    fn polygon_geometry_with_a_hole_round_trips_through_json() {
        let geometry = PlaceGeometry::Polygon {
            exterior: vec![point("60.0", "5.0"), point("61.0", "5.0"), point("61.0", "6.0")],
            holes: vec![vec![point("60.3", "5.3"), point("60.4", "5.3"), point("60.4", "5.4")]],
        };
        let json = serde_json::to_string(&geometry).unwrap();
        let back: PlaceGeometry = serde_json::from_str(&json).unwrap();
        assert_eq!(geometry, back);
    }

    #[test]
    fn point_bounding_box_is_the_point_itself() {
        let geometry = PlaceGeometry::Point(point("60.39", "5.32"));
        let (min_lat, min_lon, max_lat, max_lon) = geometry.bounding_box().unwrap();
        assert_eq!(min_lat, max_lat);
        assert_eq!(min_lon, max_lon);
        assert_eq!(min_lat, Microdegrees::from_str("60.39").unwrap());
    }

    #[test]
    fn polygon_bounding_box_spans_exterior_and_holes() {
        let geometry = PlaceGeometry::Polygon {
            exterior: vec![point("60.0", "5.0"), point("61.0", "5.0"), point("61.0", "6.0")],
            holes: vec![vec![point("59.5", "4.5"), point("59.6", "4.5"), point("59.6", "4.6")]],
        };
        let (min_lat, min_lon, max_lat, max_lon) = geometry.bounding_box().unwrap();
        assert_eq!(min_lat, Microdegrees::from_str("59.5").unwrap());
        assert_eq!(min_lon, Microdegrees::from_str("4.5").unwrap());
        assert_eq!(max_lat, Microdegrees::from_str("61.0").unwrap());
        assert_eq!(max_lon, Microdegrees::from_str("6.0").unwrap());
    }

    #[test]
    fn empty_polygon_has_no_bounding_box() {
        let geometry = PlaceGeometry::Polygon {
            exterior: Vec::new(),
            holes: Vec::new(),
        };
        assert!(geometry.bounding_box().is_none());
    }

    #[test]
    fn a_point_is_its_own_representative_point() {
        let coordinate = point("60.39", "5.32");
        let geometry = PlaceGeometry::Point(coordinate);
        assert_eq!(geometry.representative_point(), Some(coordinate));
    }

    #[test]
    fn a_polygons_representative_point_averages_the_exterior_ring() {
        let geometry = PlaceGeometry::Polygon {
            exterior: vec![point("60.0", "5.0"), point("61.0", "5.0"), point("62.0", "5.0")],
            holes: vec![vec![point("0.0", "0.0")]],
        };
        // The average ignores holes: (60+61+62)/3 = 61, longitude stays 5.0 throughout.
        assert_eq!(geometry.representative_point(), Some(point("61.0", "5.0")));
    }

    #[test]
    fn an_empty_polygon_has_no_representative_point() {
        let geometry = PlaceGeometry::Polygon {
            exterior: Vec::new(),
            holes: Vec::new(),
        };
        assert!(geometry.representative_point().is_none());
    }
}
