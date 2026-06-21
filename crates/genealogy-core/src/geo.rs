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

#[cfg(test)]
mod tests {
    use super::{GeoCoordinates, Microdegrees};
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
    fn coordinates_round_trip_through_json() {
        let point = GeoCoordinates {
            latitude: Microdegrees::from_str("60.39").unwrap(),
            longitude: Microdegrees::from_str("5.32").unwrap(),
        };
        let json = serde_json::to_string(&point).unwrap();
        let back: GeoCoordinates = serde_json::from_str(&json).unwrap();
        assert_eq!(point, back);
    }
}
