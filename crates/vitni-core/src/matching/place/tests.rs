//! The place comparator's cases, and the place assessment's table cases (ADR 0038 §2, §4).

use std::sync::LazyLock;

use proptest::prelude::{Strategy, prop, prop_assert, prop_assert_eq, proptest};
use uuid::Uuid;

use super::{ENCLOSED_SIMILARITY, compare, distance_km, distance_similarity};
use crate::enums::PlaceType;
use crate::geo::{GeoCoordinates, Microdegrees};
use crate::ids::PlaceId;
use crate::matching::name::Applied;
use crate::matching::pack::CulturePacks;
use crate::matching::profile::PlaceProfile;
use crate::matching::tests::{DATA, feature, household};
use crate::matching::{CultureId, Feature, MatchAssessment, MatchBand, MatchSettings, Outcome, assess_places};
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

fn assess(a: &PlaceProfile, b: &PlaceProfile) -> MatchAssessment {
    assess_places(a, b, &DATA, &MatchSettings::default())
}

/// A workspace place of `place_type` named `name`, enclosed by `enclosing`, nearest first.
fn place(name: &str, place_type: PlaceType, enclosing: &[PlaceId]) -> PlaceProfile {
    PlaceProfile {
        id: Some(id()),
        place_type: Some(place_type),
        enclosing: enclosing.to_vec(),
        country: Some("Norge".to_owned()),
        ..named(name)
    }
}

#[test]
fn a_farm_matched_to_its_parish_is_partial() {
    let parish = place("Ringsaker", PlaceType::Parish, &[]);
    let farm = place("Haugen", PlaceType::Farm, &[parish.id.unwrap()]);
    let assessment = assess(&farm, &parish);
    let enclosure = feature(&assessment, Feature::Enclosure);
    assert_eq!(
        enclosure.outcome,
        Outcome::Partial(ENCLOSED_SIMILARITY),
        "{assessment:#?}"
    );
    assert!(enclosure.weight > 0.0, "{enclosure:?}");
    assert_eq!(
        feature(&assess(&parish, &farm), Feature::Enclosure).outcome,
        enclosure.outcome
    );
    assert!(
        assessment.band < MatchBand::Probable,
        "a farm is not its parish: {assessment:#?}"
    );
}

#[test]
fn one_farm_recorded_twice_in_one_parish_is_probable() {
    let parish = place("Ringsaker", PlaceType::Parish, &[]).id.unwrap();
    let a = PlaceProfile {
        coordinates: Some(at(60.880, 10.700)),
        ..place("Nordaas", PlaceType::Farm, &[parish])
    };
    let b = PlaceProfile {
        coordinates: Some(at(60.881, 10.701)),
        ..place("Nordås", PlaceType::Farm, &[parish])
    };
    let assessment = assess(&a, &b);
    assert_eq!(assessment.band, MatchBand::Probable, "{assessment:#?}");
    for wanted in [
        Feature::PlaceName,
        Feature::PlaceType,
        Feature::Enclosure,
        Feature::Coordinates,
    ] {
        assert_eq!(feature(&assessment, wanted).outcome, Outcome::Agree, "{wanted:?}");
    }
    let cultures: Vec<&str> = assessment.cultures.iter().map(CultureId::as_str).collect();
    assert_eq!(cultures, ["universal", "da", "no"]);
}

#[test]
fn a_farm_under_a_parish_in_the_others_county_is_partial() {
    let county = place("Hedmark", PlaceType::County, &[]).id.unwrap();
    let parish = place("Ringsaker", PlaceType::Parish, &[county]).id.unwrap();
    let a = place("Haugen", PlaceType::Farm, &[parish, county]);
    let b = place("Haugen", PlaceType::Farm, &[county]);
    let enclosure = feature(&assess(&a, &b), Feature::Enclosure).clone();
    assert_eq!(
        enclosure.outcome,
        Outcome::Partial(ENCLOSED_SIMILARITY),
        "{enclosure:?}"
    );
}

#[test]
fn farms_of_one_name_in_different_parishes_disagree() {
    let (east, west) = (
        place("Ringsaker", PlaceType::Parish, &[]).id.unwrap(),
        place("Voss", PlaceType::Parish, &[]).id.unwrap(),
    );
    let assessment = assess(
        &place("Haugen", PlaceType::Farm, &[east]),
        &place("Haugen", PlaceType::Farm, &[west]),
    );
    assert_eq!(feature(&assessment, Feature::Enclosure).outcome, Outcome::Disagree);
    assert!(assessment.band < MatchBand::Probable, "{assessment:#?}");
}

#[test]
fn farms_of_one_name_far_apart_are_unlikely() {
    let a = PlaceProfile {
        coordinates: Some(at(60.88, 10.70)),
        ..named("Haugen")
    };
    let b = PlaceProfile {
        coordinates: Some(at(60.39, 5.32)),
        ..named("Haugen")
    };
    let assessment = assess(&a, &b);
    assert_eq!(feature(&assessment, Feature::Coordinates).outcome, Outcome::Disagree);
    assert_eq!(feature(&assessment, Feature::PlaceName).outcome, Outcome::Agree);
    assert_eq!(assessment.band, MatchBand::Unlikely, "{assessment:#?}");
}

#[test]
fn a_different_type_disagrees_and_a_custom_type_compares_case_folded() {
    let typed = |place_type: PlaceType| PlaceProfile {
        place_type: Some(place_type),
        ..named("Haugen")
    };
    let other = assess(&typed(PlaceType::Farm), &typed(PlaceType::Parish));
    assert_eq!(feature(&other, Feature::PlaceType).outcome, Outcome::Disagree);
    let custom = |text: &str| typed(PlaceType::Custom(text.to_owned()));
    let same = assess(&custom("Sokn"), &custom("sokn"));
    assert_eq!(feature(&same, Feature::PlaceType).outcome, Outcome::Agree);
    let unknown = assess(&named("Haugen"), &typed(PlaceType::Farm));
    assert_eq!(feature(&unknown, Feature::PlaceType).outcome, Outcome::Missing);
}

#[test]
fn a_dated_historical_name_matches_the_modern_one() {
    let mut old = named("Hamar");
    old.names.push(PlaceName {
        text: "Storhamar".to_owned(),
        language: None,
        date: Some(crate::matching::tests::point(1800, None, None)),
    });
    let assessment = assess(&old, &named("Storhammer"));
    assert!(feature(&assessment, Feature::PlaceName).weight > 0.0, "{assessment:#?}");
}

#[test]
fn a_shared_origin_is_deterministic_and_two_items_of_one_record_conflict() {
    let (mut a, mut b) = (named("Haugen"), named("Haugen"));
    a.origins.push(household(Some("place:1")));
    b.origins.push(household(Some("place:2")));
    let apart = assess(&a, &b);
    assert_eq!(feature(&apart, Feature::Record).outcome, Outcome::Conflict);
    b.origins = vec![household(Some("place:1"))];
    assert_eq!(assess(&a, &b).band, MatchBand::Deterministic);
}

fn a_place() -> impl Strategy<Value = PlaceProfile> {
    let ids: Vec<PlaceId> = (0..3).map(|_| id()).collect();
    let types = prop::sample::select(vec![
        PlaceType::Farm,
        PlaceType::Parish,
        PlaceType::Custom("Sokn".to_owned()),
    ]);
    (
        prop::sample::select(vec!["Haugen", "Haug", "Nordaas", "Nordås", "Ringsaker"]),
        prop::option::of(types),
        prop::sample::subsequence(ids.clone(), 0..=2),
        prop::option::of(prop::sample::select(ids)),
        prop::option::of((60.0f64..61.0, 10.0f64..11.0)),
    )
        .prop_map(|(name, place_type, enclosing, own, coordinates)| PlaceProfile {
            id: own.filter(|own| !enclosing.contains(own)),
            place_type,
            enclosing,
            coordinates: coordinates.map(|(lat, lon)| at(lat, lon)),
            ..named(name)
        })
}

proptest! {
    #[test]
    fn distance_similarity_never_rises_with_distance(a in 0.0f64..200.0, b in 0.0f64..200.0) {
        let (near, far) = if a <= b { (a, b) } else { (b, a) };
        prop_assert!(distance_similarity(near) >= distance_similarity(far));
    }

    #[test]
    fn a_place_assessment_is_a_symmetric_probability(a in a_place(), b in a_place()) {
        let (ab, ba) = (assess(&a, &b), assess(&b, &a));
        prop_assert!((0.0..=1.0).contains(&ab.score), "{}", ab.score);
        prop_assert!((ab.score - ba.score).abs() < 1e-9, "{} vs {}", ab.score, ba.score);
        prop_assert_eq!(ab.band, ba.band);
    }

    #[test]
    fn a_place_matches_itself_at_least_as_well_as_any_other(a in a_place(), b in a_place()) {
        let (itself, other) = (assess(&a, &a), assess(&a, &b));
        prop_assert!(itself.score >= other.score, "self {} < other {}", itself.score, other.score);
    }
}
