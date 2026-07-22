//! [`PlaceGeometryAssertion`] — a place's geometry, with the date it held (data-model §7, ADR 0024).
//!
//! A place's boundary moves over time — a parish's 1801 extent differs from its 1900 extent — so,
//! unlike the last-writer-wins [`GeoCoordinates`](crate::geo::GeoCoordinates) point, geometry
//! assertions **accumulate**: each dated shape coexists with the others rather than replacing them,
//! mirroring the dated enclosure link [`PlaceRef`](crate::place_ref::PlaceRef). The right one for a
//! given moment is picked by a date-selection rule (deferred to the temporal-resolution follow-up).

use serde::{Deserialize, Serialize};

use crate::date::GenealogicalDate;
use crate::geo::PlaceGeometry;

/// One geometry a place had, and the date it held (data-model §7, ADR 0024).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlaceGeometryAssertion {
    /// The asserted shape.
    pub geometry: PlaceGeometry,
    /// The date this geometry held, if known (an undated assertion is the historical `Point` case).
    pub date: Option<GenealogicalDate>,
}

#[cfg(test)]
mod tests {
    use super::PlaceGeometryAssertion;
    use crate::geo::{GeoCoordinates, Microdegrees, PlaceGeometry};
    use std::str::FromStr;

    #[test]
    fn geometry_assertion_round_trips_through_json() {
        let assertion = PlaceGeometryAssertion {
            geometry: PlaceGeometry::Point(GeoCoordinates {
                latitude: Microdegrees::from_str("60.39").unwrap(),
                longitude: Microdegrees::from_str("5.32").unwrap(),
            }),
            date: None,
        };
        let json = serde_json::to_string(&assertion).unwrap();
        let back: PlaceGeometryAssertion = serde_json::from_str(&json).unwrap();
        assert_eq!(assertion, back);
    }
}
