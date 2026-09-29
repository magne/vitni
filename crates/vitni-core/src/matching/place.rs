//! Place comparison for record matching (ADR 0038 §4): the same place, a place enclosing the other,
//! nearby coordinates, or similar names.
//!
//! Identity and enclosure come first: a farm recorded in one source and its parish in another are a
//! partial agreement, never a disagreement. Coordinates come next and decay with distance, so two farms
//! of one name in different valleys disagree. Names, compared by the applied culture packs, are the
//! fallback.

use strsim::jaro_winkler;

use crate::geo::GeoCoordinates;
use crate::matching::name::Applied;
use crate::matching::profile::PlaceProfile;
use crate::matching::weights::PLACE_NAME_FLOOR;

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
mod tests {
    use std::sync::LazyLock;

    use proptest::prelude::{prop_assert, proptest};
    use uuid::Uuid;

    use super::{ENCLOSED_SIMILARITY, compare, distance_km, distance_similarity};
    use crate::geo::{GeoCoordinates, Microdegrees};
    use crate::ids::PlaceId;
    use crate::matching::CultureId;
    use crate::matching::name::Applied;
    use crate::matching::pack::CulturePacks;
    use crate::matching::profile::PlaceProfile;
    use crate::place_name::PlaceName;

    static PACKS: LazyLock<CulturePacks> = LazyLock::new(|| CulturePacks::embedded().unwrap());

    fn norwegian() -> Applied<'static> {
        Applied::new(
            ["universal", "no"]
                .iter()
                .map(|id| PACKS.get(&CultureId::new(*id)).unwrap())
                .collect(),
        )
    }

    fn at(latitude: f64, longitude: f64) -> GeoCoordinates {
        let micro = |degrees: f64| Microdegrees::from_microdegrees(format!("{:.0}", degrees * 1e6).parse().unwrap());
        GeoCoordinates {
            latitude: micro(latitude),
            longitude: micro(longitude),
        }
    }

    fn named(name: &str) -> PlaceProfile {
        PlaceProfile {
            names: vec![PlaceName {
                text: name.to_owned(),
                language: None,
                date: None,
            }],
            ..PlaceProfile::default()
        }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() < f64::EPSILON
    }

    fn id() -> PlaceId {
        PlaceId::from_uuid(Uuid::now_v7())
    }

    #[test]
    fn the_same_place_agrees() {
        let place = PlaceProfile {
            id: Some(id()),
            ..named("Nordaas")
        };
        assert!(close(
            compare(&place, &place.clone(), &norwegian()).unwrap().similarity,
            1.0
        ));
    }

    #[test]
    fn a_farm_in_its_parish_is_partial() {
        let parish = PlaceProfile {
            id: Some(id()),
            ..named("Ringsaker")
        };
        let farm = PlaceProfile {
            id: Some(id()),
            enclosing: vec![parish.id.unwrap()],
            ..named("Haugen")
        };
        let graded = compare(&farm, &parish, &norwegian()).unwrap();
        assert!(close(graded.similarity, ENCLOSED_SIMILARITY));
        assert_eq!(compare(&parish, &farm, &norwegian()), Some(graded));
    }

    #[test]
    fn two_farms_of_one_name_far_apart_disagree() {
        let east = PlaceProfile {
            coordinates: Some(at(60.88, 10.70)),
            ..named("Haugen")
        };
        let west = PlaceProfile {
            coordinates: Some(at(60.39, 5.32)),
            ..named("Haugen")
        };
        assert!(close(compare(&east, &west, &norwegian()).unwrap().similarity, 0.0));
    }

    #[test]
    fn spelling_variants_of_a_place_name_agree() {
        let graded = compare(&named("Nordaas"), &named("Nordås"), &norwegian()).unwrap();
        assert!(close(graded.similarity, 1.0));
        assert_eq!(compare(&named("Nordaas"), &PlaceProfile::default(), &norwegian()), None);
    }

    #[test]
    fn oslo_to_bergen_is_about_three_hundred_km() {
        let km = distance_km(at(59.91, 10.75), at(60.39, 5.32));
        assert!((300.0..310.0).contains(&km), "{km}");
    }

    proptest! {
        #[test]
        fn distance_similarity_never_rises_with_distance(a in 0.0f64..200.0, b in 0.0f64..200.0) {
            let (near, far) = if a <= b { (a, b) } else { (b, a) };
            prop_assert!(distance_similarity(near) >= distance_similarity(far));
        }
    }
}
