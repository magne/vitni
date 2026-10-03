//! The matching engine's `query.find-similar` across the WIT boundary (ADR 0038 §8): the WIT band onto
//! the engine's, and each similar record with its assessment onto the WIT `similar-record`.

use vitni_app::{Feature, FeatureComparison, FeatureValue, MatchBand, MediaPath, Outcome, SimilarRecord, VitalKind};
use vitni_core::enums::EventType;

use crate::bindings::imports::vitni::host_api::types;
use crate::state::{from_address, from_event_type, from_genealogical_date, from_place_type, from_sex};

/// Maps the WIT `match-band` onto the engine's [`MatchBand`].
pub(crate) fn to_band(band: types::MatchBand) -> MatchBand {
    match band {
        types::MatchBand::Unlikely => MatchBand::Unlikely,
        types::MatchBand::Possible => MatchBand::Possible,
        types::MatchBand::Probable => MatchBand::Probable,
        types::MatchBand::Deterministic => MatchBand::Deterministic,
    }
}

/// Maps a record the engine judged similar onto the WIT `similar-record`.
pub(crate) fn from_similar(similar: &SimilarRecord) -> types::SimilarRecord {
    let assessment = &similar.assessment;
    let mut features = Vec::with_capacity(assessment.features.len());
    for term in &assessment.features {
        features.push(from_term(term));
    }
    let mut cultures = Vec::with_capacity(assessment.cultures.len());
    for culture in &assessment.cultures {
        cultures.push(culture.as_str().to_owned());
    }
    types::SimilarRecord {
        human_id: similar.record.human_id.clone(),
        score: assessment.score,
        band: from_band(assessment.band),
        features,
        cultures,
        engine: assessment.engine.0,
    }
}

fn from_band(band: MatchBand) -> types::MatchBand {
    match band {
        MatchBand::Unlikely => types::MatchBand::Unlikely,
        MatchBand::Possible => types::MatchBand::Possible,
        MatchBand::Probable => types::MatchBand::Probable,
        MatchBand::Deterministic => types::MatchBand::Deterministic,
    }
}

fn from_term(term: &FeatureComparison) -> types::FeatureComparison {
    types::FeatureComparison {
        feature: from_feature(term.feature),
        outcome: from_outcome(term.outcome),
        weight: term.weight,
        left: term.left.as_ref().map(from_value),
        right: term.right.as_ref().map(from_value),
    }
}

fn from_outcome(outcome: Outcome) -> types::MatchOutcome {
    match outcome {
        Outcome::Agree => types::MatchOutcome::Agree,
        Outcome::Partial(similarity) => types::MatchOutcome::Partial(similarity),
        Outcome::Disagree => types::MatchOutcome::Disagree,
        Outcome::Missing => types::MatchOutcome::Missing,
        Outcome::Conflict => types::MatchOutcome::Conflict,
    }
}

fn from_value(value: &FeatureValue) -> types::FeatureValue {
    match value {
        FeatureValue::Name(name) => types::FeatureValue::Name(name.clone()),
        FeatureValue::Date { kind, date } => types::FeatureValue::Date(types::VitalDate {
            kind: from_vital_kind(*kind),
            date: from_genealogical_date(date),
        }),
        FeatureValue::Place(place) => types::FeatureValue::Place(place.clone()),
        FeatureValue::When(date) => types::FeatureValue::When(from_genealogical_date(date)),
        FeatureValue::EventType(EventType::Custom(name)) => types::FeatureValue::Text(name.clone()),
        FeatureValue::EventType(event_type) => from_event_type(event_type).map_or_else(
            || types::FeatureValue::Text(String::new()),
            types::FeatureValue::EventType,
        ),
        FeatureValue::Sex(sex) => types::FeatureValue::Sex(from_sex(sex)),
        FeatureValue::Relative { name, born } => types::FeatureValue::Relative(types::RelativeValue {
            name: name.clone(),
            born: born.as_ref().map(from_genealogical_date),
        }),
        FeatureValue::Text(text) => types::FeatureValue::Text(text.clone()),
        FeatureValue::Origin(origin) => types::FeatureValue::Origin(types::OriginKey {
            record: origin.record.clone(),
            item: origin.item.clone(),
        }),
        FeatureValue::PlaceType(place_type) => types::FeatureValue::PlaceType(from_place_type(place_type.clone())),
        FeatureValue::Coordinates(point) => types::FeatureValue::Coordinates(types::Coordinates {
            latitude: point.latitude.to_degrees(),
            longitude: point.longitude.to_degrees(),
        }),
        FeatureValue::Address(address) => types::FeatureValue::Address(from_address(address)),
        FeatureValue::Path(path) => types::FeatureValue::Path(match path {
            MediaPath::File(file) => file.clone(),
            MediaPath::Web(url) => url.href.clone(),
        }),
    }
}

fn from_vital_kind(kind: VitalKind) -> types::VitalKind {
    match kind {
        VitalKind::Birth => types::VitalKind::Birth,
        VitalKind::Baptism => types::VitalKind::Baptism,
        VitalKind::Death => types::VitalKind::Death,
        VitalKind::Burial => types::VitalKind::Burial,
    }
}

fn from_feature(feature: Feature) -> types::MatchFeature {
    match feature {
        Feature::GivenName => types::MatchFeature::GivenName,
        Feature::Surname => types::MatchFeature::Surname,
        Feature::Sex => types::MatchFeature::Sex,
        Feature::Birth => types::MatchFeature::Birth,
        Feature::Death => types::MatchFeature::Death,
        Feature::BirthPlace => types::MatchFeature::BirthPlace,
        Feature::DeathPlace => types::MatchFeature::DeathPlace,
        Feature::Lifespan => types::MatchFeature::Lifespan,
        Feature::Father => types::MatchFeature::Father,
        Feature::Mother => types::MatchFeature::Mother,
        Feature::Partners => types::MatchFeature::Partners,
        Feature::Children => types::MatchFeature::Children,
        Feature::Patronymic => types::MatchFeature::Patronymic,
        Feature::Occupation => types::MatchFeature::Occupation,
        Feature::Record => types::MatchFeature::Record,
        Feature::Partner => types::MatchFeature::Partner,
        Feature::Marriage => types::MatchFeature::Marriage,
        Feature::MarriagePlace => types::MatchFeature::MarriagePlace,
        Feature::EventType => types::MatchFeature::EventType,
        Feature::Date => types::MatchFeature::Date,
        Feature::Place => types::MatchFeature::Place,
        Feature::Principal => types::MatchFeature::Principal,
        Feature::Participants => types::MatchFeature::Participants,
        Feature::PlaceName => types::MatchFeature::PlaceName,
        Feature::PlaceType => types::MatchFeature::PlaceType,
        Feature::Enclosure => types::MatchFeature::Enclosure,
        Feature::Coordinates => types::MatchFeature::Coordinates,
        Feature::Title => types::MatchFeature::Title,
        Feature::Author => types::MatchFeature::Author,
        Feature::Publication => types::MatchFeature::Publication,
        Feature::Repository => types::MatchFeature::Repository,
        Feature::Name => types::MatchFeature::Name,
        Feature::Address => types::MatchFeature::Address,
        Feature::Source => types::MatchFeature::Source,
        Feature::Page => types::MatchFeature::Page,
        Feature::Checksum => types::MatchFeature::Checksum,
        Feature::Path => types::MatchFeature::Path,
        Feature::Text => types::MatchFeature::Text,
    }
}

#[cfg(test)]
mod tests {
    use vitni_app::{AggRef, EngineVersion, MatchAssessment};
    use vitni_core::date::{Calendar, DateQuality, GenealogicalDate, GenealogicalDateBody};
    use vitni_core::geo::{GeoCoordinates, Microdegrees};
    use vitni_core::matching::CultureId;
    use vitni_core::text::Url;

    use super::*;

    fn term(
        feature: Feature,
        outcome: Outcome,
        left: Option<FeatureValue>,
        right: Option<FeatureValue>,
    ) -> FeatureComparison {
        FeatureComparison {
            feature,
            outcome,
            weight: 1.5,
            left,
            right,
        }
    }

    fn similar(features: Vec<FeatureComparison>) -> SimilarRecord {
        SimilarRecord {
            record: AggRef {
                human_id: "I0002".to_owned(),
                id: "0190f0a0-0000-7000-8000-000000000002".to_owned(),
            },
            assessment: MatchAssessment {
                score: 0.875,
                band: MatchBand::Probable,
                features,
                cultures: vec![CultureId::new("universal"), CultureId::new("no")],
                parts: Vec::new(),
                engine: EngineVersion(4),
            },
        }
    }

    fn text_date(text: &str) -> GenealogicalDate {
        GenealogicalDate {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            modifier: GenealogicalDateBody::TextOnly { text: text.to_owned() },
            time: None,
            new_year_begins: None,
            sort_value: 0,
            original_text: None,
        }
    }

    #[test]
    fn every_band_maps_onto_the_engines() {
        assert_eq!(to_band(types::MatchBand::Unlikely), MatchBand::Unlikely);
        assert_eq!(to_band(types::MatchBand::Possible), MatchBand::Possible);
        assert_eq!(to_band(types::MatchBand::Probable), MatchBand::Probable);
        assert_eq!(to_band(types::MatchBand::Deterministic), MatchBand::Deterministic);
    }

    #[test]
    fn a_similar_record_keeps_its_id_score_band_cultures_and_engine() {
        let mapped = from_similar(&similar(Vec::new()));

        assert_eq!(mapped.human_id, "I0002");
        assert!((mapped.score - 0.875).abs() < f64::EPSILON);
        assert_eq!(mapped.band, types::MatchBand::Probable);
        assert_eq!(mapped.cultures, ["universal", "no"]);
        assert_eq!(mapped.engine, 4);
        assert!(mapped.features.is_empty());
    }

    #[test]
    fn a_term_keeps_its_feature_outcome_weight_and_both_values() {
        let mapped = from_similar(&similar(vec![
            term(
                Feature::GivenName,
                Outcome::Partial(0.75),
                Some(FeatureValue::Name("Ole".to_owned())),
                Some(FeatureValue::Name("Ola".to_owned())),
            ),
            term(Feature::Record, Outcome::Conflict, None, None),
        ]));

        let given = &mapped.features[0];
        assert_eq!(given.feature, types::MatchFeature::GivenName);
        assert!(matches!(given.outcome, types::MatchOutcome::Partial(s) if (s - 0.75).abs() < f64::EPSILON));
        assert!((given.weight - 1.5).abs() < f64::EPSILON);
        assert!(matches!(&given.left, Some(types::FeatureValue::Name(name)) if name == "Ole"));
        assert!(matches!(&given.right, Some(types::FeatureValue::Name(name)) if name == "Ola"));
        let record = &mapped.features[1];
        assert_eq!(record.feature, types::MatchFeature::Record);
        assert!(matches!(record.outcome, types::MatchOutcome::Conflict));
        assert!(record.left.is_none() && record.right.is_none());
    }

    #[test]
    fn a_vital_date_keeps_which_event_it_is_of() {
        let mapped = from_similar(&similar(vec![term(
            Feature::Birth,
            Outcome::Agree,
            Some(FeatureValue::Date {
                kind: VitalKind::Baptism,
                date: text_date("1850"),
            }),
            None,
        )]));

        let Some(types::FeatureValue::Date(vital)) = &mapped.features[0].left else {
            panic!("expected a vital date, got {:?}", mapped.features[0].left);
        };
        assert_eq!(vital.kind, types::VitalKind::Baptism);
        assert!(matches!(&vital.date.modifier, types::DateModifier::TextOnly(text) if text == "1850"));
    }

    #[test]
    fn a_custom_event_type_is_text_and_a_known_one_its_enum() {
        let mapped = from_similar(&similar(vec![term(
            Feature::EventType,
            Outcome::Disagree,
            Some(FeatureValue::EventType(EventType::Baptism)),
            Some(FeatureValue::EventType(EventType::Custom("Vaksinasjon".to_owned()))),
        )]));

        let term = &mapped.features[0];
        assert!(matches!(
            term.left,
            Some(types::FeatureValue::EventType(types::EventType::Baptism))
        ));
        assert!(matches!(&term.right, Some(types::FeatureValue::Text(text)) if text == "Vaksinasjon"));
    }

    #[test]
    fn coordinates_are_degrees_and_a_media_path_its_file_or_url() {
        let point = GeoCoordinates {
            latitude: Microdegrees::from_microdegrees(59_913_900),
            longitude: Microdegrees::from_microdegrees(10_752_200),
        };
        let mapped = from_similar(&similar(vec![
            term(
                Feature::Coordinates,
                Outcome::Agree,
                Some(FeatureValue::Coordinates(point)),
                None,
            ),
            term(
                Feature::Path,
                Outcome::Missing,
                Some(FeatureValue::Path(MediaPath::File("media/a.jpg".to_owned()))),
                Some(FeatureValue::Path(MediaPath::Web(Url {
                    url_type: None,
                    href: "https://example.org/scan.jpg".to_owned(),
                    description: None,
                }))),
            ),
        ]));

        let Some(types::FeatureValue::Coordinates(at)) = &mapped.features[0].left else {
            panic!("expected coordinates, got {:?}", mapped.features[0].left);
        };
        assert!((at.latitude - 59.9139).abs() < 1e-9 && (at.longitude - 10.7522).abs() < 1e-9);
        let path = &mapped.features[1];
        assert!(matches!(&path.left, Some(types::FeatureValue::Path(p)) if p == "media/a.jpg"));
        assert!(matches!(&path.right, Some(types::FeatureValue::Path(p)) if p == "https://example.org/scan.jpg"));
    }
}
