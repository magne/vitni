//! Place comparison for record matching (ADR 0038 §2, §4): the same place, a place enclosing the other,
//! nearby coordinates, or similar names.
//!
//! As the place of a person's or an event's event ([`compare`]), a place is one graded term. Identity
//! and enclosure come first: a farm recorded in one source and its parish in another are a partial
//! agreement, never a disagreement. Coordinates come next and decay with distance, so two farms of one
//! name in different valleys disagree. Names, compared by the applied culture packs, are the fallback.
//!
//! As a record of its own ([`assess_places`]), a place is compared feature by feature: its names — every
//! dated, historical one among them — its type, where it lies and its coordinates. Where it lies is the
//! same place, one place enclosing the other (a farm and its parish: partial), the same nearest
//! enclosing place (agreement), one's nearest enclosing place in the other's chain (partial), or two
//! different enclosing places (disagreement).

use strsim::jaro_winkler;

use crate::enums::PlaceType;
use crate::geo::GeoCoordinates;
use crate::matching::name::{Applied, fold};
use crate::matching::profile::PlaceProfile;
use crate::matching::select::{Signals, comparison_cultures};
use crate::matching::weights::{self, PLACE_NAME_FLOOR};
use crate::matching::{
    Feature, FeatureComparison, FeatureValue, Identity, MatchAssessment, MatchData, MatchSettings, applied, conclude,
    grade, missing,
};

/// The similarity of a place and a place enclosing it.
pub(crate) const ENCLOSED_SIMILARITY: f64 = 0.8;

/// Within this distance two coordinates agree.
const AGREE_KM: f64 = 1.0;

/// The distance past [`AGREE_KM`] at which similarity has halved.
const HALF_KM: f64 = 10.0;

/// Beyond this distance two coordinates disagree.
const LIMIT_KM: f64 = 50.0;

/// The mean radius of the Earth.
const EARTH_RADIUS_KM: f64 = 6371.0;

/// A similarity in `0..=1` and the floor below which it is a disagreement.
#[derive(Debug, Clone, Copy, PartialEq)]
pub(crate) struct Graded {
    pub similarity: f64,
    pub floor: f64,
}

/// How similar two places are, or `None` when there is nothing to compare.
pub(crate) fn compare(a: &PlaceProfile, b: &PlaceProfile, applied: &Applied<'_>) -> Option<Graded> {
    if let (Some(x), Some(y)) = (a.id, b.id) {
        if x == y {
            return Some(Graded {
                similarity: 1.0,
                floor: 0.0,
            });
        }
        if a.enclosing.contains(&y) || b.enclosing.contains(&x) {
            return Some(Graded {
                similarity: ENCLOSED_SIMILARITY,
                floor: 0.0,
            });
        }
    }
    if let (Some(x), Some(y)) = (a.coordinates, b.coordinates) {
        return Some(Graded {
            similarity: distance_similarity(distance_km(x, y)),
            floor: 0.0,
        });
    }
    names_similarity(a, b, applied).map(|similarity| Graded {
        similarity,
        floor: PLACE_NAME_FLOOR,
    })
}

/// Compares two place profiles.
#[must_use]
pub fn assess_places(
    a: &PlaceProfile,
    b: &PlaceProfile,
    data: &MatchData,
    settings: &MatchSettings,
) -> MatchAssessment {
    let cultures = comparison_cultures(&Signals::place(a), &Signals::place(b), data, settings);
    let applied = applied(&cultures, data);
    let features = vec![
        compare_names(a, b, &applied),
        compare_types(a.place_type.as_ref(), b.place_type.as_ref()),
        compare_enclosure(a, b),
        compare_coordinates(a.coordinates, b.coordinates),
    ];
    let sides = (Identity::of(&a.origins, &[]), Identity::of(&b.origins, &[]));
    conclude(features, cultures, Vec::new(), sides, settings)
}

/// A term from a graded similarity, or a missing one when there is nothing to compare.
fn term(
    feature: Feature,
    graded: Option<(f64, f64)>,
    table: weights::Weights,
    (left, right): (Option<FeatureValue>, Option<FeatureValue>),
) -> FeatureComparison {
    let Some((similarity, floor)) = graded else {
        return missing(feature, left, right);
    };
    let (outcome, weight) = grade(similarity, floor, table);
    FeatureComparison {
        feature,
        outcome,
        weight,
        left,
        right,
    }
}

/// Compares every name of `a` with every name of `b`.
fn compare_names(a: &PlaceProfile, b: &PlaceProfile, applied: &Applied<'_>) -> FeatureComparison {
    let value = |p: &PlaceProfile| p.names.first().map(|name| FeatureValue::Place(name.text.clone()));
    let graded = names_similarity(a, b, applied).map(|similarity| (similarity, PLACE_NAME_FLOOR));
    term(Feature::PlaceName, graded, weights::PLACE_NAME, (value(a), value(b)))
}

/// Compares two place types; a custom type is compared by its folded text.
fn compare_types(a: Option<&PlaceType>, b: Option<&PlaceType>) -> FeatureComparison {
    let key = |place_type: &PlaceType| {
        if let PlaceType::Custom(text) = place_type {
            Err(fold(text.trim()))
        } else {
            Ok(place_type.clone())
        }
    };
    let graded = a.zip(b).map(|(x, y)| (if key(x) == key(y) { 1.0 } else { 0.0 }, 0.0));
    let value = |place_type: &PlaceType| FeatureValue::PlaceType(place_type.clone());
    term(
        Feature::PlaceType,
        graded,
        weights::PLACE_TYPE,
        (a.map(value), b.map(value)),
    )
}

/// Compares where two places lie.
fn compare_enclosure(a: &PlaceProfile, b: &PlaceProfile) -> FeatureComparison {
    let enclosed = |x: &PlaceProfile, y: &PlaceProfile| y.id.is_some_and(|id| x.enclosing.contains(&id));
    let similarity = if a.id.is_some() && a.id == b.id {
        Some(1.0)
    } else if enclosed(a, b) || enclosed(b, a) {
        Some(ENCLOSED_SIMILARITY)
    } else {
        match (a.enclosing.first(), b.enclosing.first()) {
            (Some(x), Some(y)) if x == y => Some(1.0),
            (Some(x), Some(y)) if b.enclosing.contains(x) || a.enclosing.contains(y) => Some(ENCLOSED_SIMILARITY),
            (Some(_), Some(_)) => Some(0.0),
            (None, _) | (_, None) => None,
        }
    };
    let value = |p: &PlaceProfile| p.names.first().map(|name| FeatureValue::Place(name.text.clone()));
    term(
        Feature::Enclosure,
        similarity.map(|s| (s, 0.0)),
        weights::ENCLOSURE,
        (value(a), value(b)),
    )
}

/// Compares two coordinates by their distance.
fn compare_coordinates(a: Option<GeoCoordinates>, b: Option<GeoCoordinates>) -> FeatureComparison {
    let graded = a.zip(b).map(|(x, y)| (distance_similarity(distance_km(x, y)), 0.0));
    term(
        Feature::Coordinates,
        graded,
        weights::COORDINATES,
        (a.map(FeatureValue::Coordinates), b.map(FeatureValue::Coordinates)),
    )
}

/// The best similarity between any name of `a` and any name of `b`.
fn names_similarity(a: &PlaceProfile, b: &PlaceProfile, applied: &Applied<'_>) -> Option<f64> {
    let mut best: Option<f64> = None;
    for x in &a.names {
        for y in &b.names {
            let (x, y) = (applied.tokens(&x.text).join(" "), applied.tokens(&y.text).join(" "));
            if x.is_empty() || y.is_empty() {
                continue;
            }
            let similarity = if x == y {
                1.0
            } else {
                jaro_winkler(&x, &y).max(jaro_winkler(&y, &x))
            };
            best = Some(best.map_or(similarity, |so_far: f64| so_far.max(similarity)));
        }
    }
    best
}

/// The similarity of two points `km` apart: `1` within a kilometre, halving over the next ten, and
/// `0` beyond fifty.
pub(crate) fn distance_similarity(km: f64) -> f64 {
    if km <= AGREE_KM {
        1.0
    } else if km > LIMIT_KM {
        0.0
    } else {
        1.0 / (1.0 + ((km - AGREE_KM) / HALF_KM).powi(2))
    }
}

/// The great-circle distance between two points.
pub(crate) fn distance_km(a: GeoCoordinates, b: GeoCoordinates) -> f64 {
    let (lat_a, lat_b) = (
        a.latitude.to_degrees().to_radians(),
        b.latitude.to_degrees().to_radians(),
    );
    let d_lat = lat_b - lat_a;
    let d_lon = (b.longitude.to_degrees() - a.longitude.to_degrees()).to_radians();
    let h = (d_lat / 2.0).sin().powi(2) + lat_a.cos() * lat_b.cos() * (d_lon / 2.0).sin().powi(2);
    2.0 * EARTH_RADIUS_KM * h.sqrt().min(1.0).asin()
}

#[cfg(test)]
mod tests;
