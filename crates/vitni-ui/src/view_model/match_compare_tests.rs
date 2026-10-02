use super::{CompareSide, MatchCompareVm, MediaRefVm, RowOutcome};
use crate::i18n::Localizer;
use uuid::Uuid;
use vitni_app::{
    DatasetId, DateParts, Feature, FeatureComparison, FeatureValue, GeoCoordinates, ImportRunId, MatchAssessment,
    MatchBand, Microdegrees, Outcome, PlaceType, RecordOrigin, Rect, VitalKind, gregorian_date,
};

fn year(value: i32) -> vitni_app::GenealogicalDate {
    gregorian_date(DateParts {
        year: value,
        month: None,
        day: None,
    })
}

fn term(
    feature: Feature,
    outcome: Outcome,
    weight: f64,
    left: Option<FeatureValue>,
    right: Option<FeatureValue>,
) -> FeatureComparison {
    FeatureComparison {
        feature,
        outcome,
        weight,
        left,
        right,
    }
}

fn assessment(features: Vec<FeatureComparison>) -> MatchAssessment {
    MatchAssessment {
        score: 0.87,
        band: MatchBand::Probable,
        features,
        cultures: Vec::new(),
        parts: Vec::new(),
        engine: vitni_app::ENGINE_VERSION,
    }
}

fn side<'a>(human_id: &'a str, label: &'a str) -> CompareSide<'a> {
    CompareSide {
        human_id,
        label,
        origin: None,
        media: &[],
    }
}

fn media(human_id: &str, path: &str, crop: Option<Rect>) -> MediaRefVm {
    MediaRefVm {
        human_id: human_id.to_owned(),
        assertion_id: format!("{human_id}-attach"),
        caption: None,
        crop,
        path: Some(path.to_owned()),
        mime: None,
    }
}

fn name(value: &str) -> FeatureValue {
    FeatureValue::Name(value.to_owned())
}

#[test]
fn a_person_pair_gets_one_row_per_compared_feature_in_the_engines_order() {
    let loc = Localizer::for_test("en");
    let features = vec![
        term(
            Feature::GivenName,
            Outcome::Agree,
            3.0,
            Some(name("Ole")),
            Some(name("Ole")),
        ),
        term(
            Feature::Birth,
            Outcome::Partial(0.6),
            2.1,
            Some(FeatureValue::Date {
                kind: VitalKind::Birth,
                date: year(1852),
            }),
            Some(FeatureValue::Date {
                kind: VitalKind::Baptism,
                date: year(1849),
            }),
        ),
        term(Feature::Surname, Outcome::Missing, 0.0, Some(name("Hansen")), None),
        term(Feature::Sex, Outcome::Conflict, -9.0, None, None),
        term(Feature::Death, Outcome::Missing, 0.0, None, None),
    ];
    let assessed = assessment(features);
    let vm = MatchCompareVm::build(side("I0001", "Ole Hansen"), side("I0002", "Ole"), &assessed, &loc);

    let rows: Vec<(&str, Option<&str>, Option<&str>, RowOutcome)> = vm
        .rows
        .iter()
        .map(|row| {
            (
                row.feature.as_str(),
                row.left.as_deref(),
                row.right.as_deref(),
                row.outcome,
            )
        })
        .collect();
    assert_eq!(
        rows,
        [
            ("Given name", Some("Ole"), Some("Ole"), RowOutcome::Agree),
            ("Birth", Some("1852"), Some("1849 (Baptism)"), RowOutcome::Partial),
            ("Surname", Some("Hansen"), None, RowOutcome::Missing),
            ("Sex", None, None, RowOutcome::Conflict),
        ],
        "a term neither side has a value for is left out"
    );
    let explanations: Vec<&str> = vm.rows.iter().map(|row| row.explanation.as_str()).collect();
    assert_eq!(
        explanations,
        [
            "Same given name (+3.0)",
            "Similar birth (+2.1)",
            "No surname to compare",
            "Conflicting sex (\u{2212}9.0)",
        ]
    );
    assert_eq!(vm.left.label, "Ole Hansen");
    assert_eq!(vm.right.human_id, "I0002");
    assert_eq!(vm.assessment_line, "Matched at 87% · probable match · engine 4");
    assert_eq!(
        vm.assessment,
        assessed.evidence(),
        "the decision records what the view showed"
    );
}

#[test]
fn a_place_pair_gets_the_places_own_rows() {
    let loc = Localizer::for_test("en");
    let coordinates = |lat: i32, lon: i32| {
        Some(FeatureValue::Coordinates(GeoCoordinates {
            latitude: Microdegrees::from_microdegrees(lat),
            longitude: Microdegrees::from_microdegrees(lon),
        }))
    };
    let features = vec![
        term(
            Feature::PlaceName,
            Outcome::Agree,
            5.0,
            Some(FeatureValue::Place("Mandal".to_owned())),
            Some(FeatureValue::Place("Mandal".to_owned())),
        ),
        term(
            Feature::PlaceType,
            Outcome::Agree,
            1.0,
            Some(FeatureValue::PlaceType(PlaceType::Municipality)),
            Some(FeatureValue::PlaceType(PlaceType::Municipality)),
        ),
        term(
            Feature::Coordinates,
            Outcome::Partial(0.9),
            1.5,
            coordinates(58_029_400, 7_460_900),
            coordinates(58_030_000, 7_461_000),
        ),
    ];
    let vm = MatchCompareVm::build(
        side("P0001", "Mandal"),
        side("P0002", "Mandal"),
        &assessment(features),
        &loc,
    );

    let rows: Vec<(&str, Option<&str>)> = vm
        .rows
        .iter()
        .map(|row| (row.feature.as_str(), row.left.as_deref()))
        .collect();
    assert_eq!(
        rows,
        [
            ("Place name", Some("Mandal")),
            ("Place type", Some("Municipality")),
            ("Coordinates", Some("58.0294, 7.4609")),
        ]
    );
}

#[test]
fn an_imported_side_carries_an_origin_chip_and_a_hand_made_one_none() {
    let loc = Localizer::for_test("en");
    let origin = RecordOrigin {
        dataset: DatasetId::new("digitalarkivet".to_owned()),
        record: "pf01073012345".to_owned(),
        item: None,
        digest: None,
        run: ImportRunId::from_uuid(Uuid::from_u128(5)),
    };
    let imported = CompareSide {
        origin: Some(&origin),
        ..side("I0001", "Ole")
    };
    let vm = MatchCompareVm::build(imported, side("I0002", "Ole"), &assessment(Vec::new()), &loc);

    let chip = vm.left.origin.expect("an origin chip");
    assert_eq!(chip.label, "digitalarkivet · pf01073012345");
    assert_eq!(
        chip.title,
        "Imported from record pf01073012345 of dataset digitalarkivet"
    );
    assert_eq!(vm.right.origin, None, "a record entered by hand has no origin");
}

#[test]
fn the_evidence_snippet_is_the_first_cropped_image() {
    let loc = Localizer::for_test("en");
    let line = Rect {
        left: 5,
        top: 40,
        width: 90,
        height: 4,
    };
    let refs = [
        media("O0001", "media/notes.pdf", Some(line)),
        media("O0002", "media/portrait.png", None),
        media("O0003", "media/scan.png", Some(line)),
    ];
    let with_scan = CompareSide {
        media: &refs,
        ..side("I0001", "Ole")
    };
    let only_uncropped = CompareSide {
        media: &refs[..2],
        ..side("I0002", "Ole")
    };
    let vm = MatchCompareVm::build(with_scan, only_uncropped, &assessment(Vec::new()), &loc);

    let snippet = vm.left.evidence.expect("the cropped scan");
    assert_eq!(snippet.src, "/media/scan.png");
    assert_eq!(snippet.crop_css, "left:5%;top:40%;width:90%;height:4%");
    assert_eq!(snippet.caption, "Evidence: O0003");
    assert_eq!(vm.right.evidence, None, "no image with a region");
}

#[test]
fn the_rows_are_localized() {
    let loc = Localizer::for_test("no");
    let features = vec![
        term(
            Feature::GivenName,
            Outcome::Agree,
            3.0,
            Some(name("Ole")),
            Some(name("Ole")),
        ),
        term(Feature::Surname, Outcome::Missing, 0.0, Some(name("Hansen")), None),
    ];
    let vm = MatchCompareVm::build(side("I0001", "Ole"), side("I0002", "Ole"), &assessment(features), &loc);

    let rows: Vec<(&str, &str)> = vm
        .rows
        .iter()
        .map(|row| (row.feature.as_str(), row.explanation.as_str()))
        .collect();
    assert_eq!(
        rows,
        [
            ("Fornavn", "Fornavn stemmer (+3,0)"),
            ("Etternavn", "Etternavn kan ikke sammenlignes"),
        ]
    );
}
