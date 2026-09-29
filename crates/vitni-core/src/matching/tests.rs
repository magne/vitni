//! The assessment's table cases and properties (ADR 0038 §3–§6).

use std::sync::LazyLock;

use proptest::prelude::{Just, Strategy, prop, prop_assert, prop_assert_eq, prop_oneof, proptest};
use uuid::Uuid;

use crate::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use crate::enums::Sex;
use crate::ids::ImportRunId;
use crate::matching::pack::PackSource;
use crate::matching::profile::{PersonProfile, PlaceProfile, VitalEvent, VitalKind};
use crate::matching::{
    CultureId, DateBasis, ENGINE_VERSION, Feature, FeatureComparison, MatchAssessment, MatchBand, MatchData,
    MatchSettings, Outcome, assess_persons,
};
use crate::name::{LanguageTag, NameType, PersonName, Surname};
use crate::origin::{DatasetId, RecordOrigin};
use crate::text::ExternalId;

static DATA: LazyLock<MatchData> = LazyLock::new(|| MatchData::embedded().unwrap());

fn assess(a: &PersonProfile, b: &PersonProfile) -> MatchAssessment {
    assess_persons(a, b, &DATA, &MatchSettings::default())
}

fn feature(assessment: &MatchAssessment, wanted: Feature) -> &FeatureComparison {
    assessment.features.iter().find(|f| f.feature == wanted).unwrap()
}

fn point(year: i32, month: Option<u8>, day: Option<u8>) -> GenealogicalDate {
    GenealogicalDate {
        calendar: Calendar::Gregorian,
        quality: DateQuality::Normal,
        modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
            year: Some(year),
            month,
            day,
        })),
        time: None,
        new_year_begins: None,
        sort_value: 0,
        original_text: None,
    }
}

fn on(year: i32, month: u8, day: u8) -> GenealogicalDate {
    point(year, Some(month), Some(day))
}

fn name(given: &str, surname: &str) -> PersonName {
    let surnames = if surname.is_empty() {
        Vec::new()
    } else {
        vec![Surname {
            prefix: None,
            surname: surname.to_owned(),
            primary: true,
            connector: None,
        }]
    };
    PersonName {
        name_type: NameType::BirthName,
        given: (!given.is_empty()).then(|| given.to_owned()),
        surnames,
        suffix: None,
        title: None,
        nickname: None,
        call_name: None,
        date: None,
        language: None,
        transliterations: Vec::new(),
    }
}

fn vital(kind: VitalKind, date: GenealogicalDate, basis: DateBasis, country: Option<&str>) -> VitalEvent {
    let place = country.map(|c| PlaceProfile {
        country: Some(c.to_owned()),
        ..PlaceProfile::default()
    });
    VitalEvent {
        kind,
        date: Some(date),
        basis,
        place,
    }
}

fn person(given: &str, surname: &str, sex: Sex, vitals: Vec<VitalEvent>) -> PersonProfile {
    PersonProfile {
        names: vec![name(given, surname)],
        sex: Some(sex),
        vitals,
        ..PersonProfile::default()
    }
}

fn born(date: GenealogicalDate, country: Option<&str>) -> Vec<VitalEvent> {
    vec![vital(VitalKind::Birth, date, DateBasis::Recorded, country)]
}

#[test]
fn guldbrand_olsen_and_gulbrand_olsøn_are_probable_in_norway() {
    let a = person("Guldbrand", "Olsen", Sex::Male, born(on(1850, 3, 4), Some("Norge")));
    let b = person("Gulbrand", "Olsøn", Sex::Male, born(on(1850, 3, 4), Some("Norge")));
    let assessment = assess(&a, &b);
    assert_eq!(assessment.band, MatchBand::Probable, "{assessment:#?}");
    assert!(matches!(feature(&assessment, Feature::GivenName).outcome, Outcome::Partial(s) if s >= 0.95));
    assert_eq!(feature(&assessment, Feature::Surname).outcome, Outcome::Agree);
    let cultures: Vec<&str> = assessment.cultures.iter().map(CultureId::as_str).collect();
    assert_eq!(cultures, ["universal", "da", "no"]);
    assert_eq!(assessment.engine, ENGINE_VERSION);
}

#[test]
fn without_a_norwegian_signal_olsøn_is_only_close_to_olsen() {
    let a = person("Gulbrand", "Olsen", Sex::Male, born(on(1850, 3, 4), None));
    let b = person("Gulbrand", "Olsøn", Sex::Male, born(on(1850, 3, 4), None));
    let assessment = assess(&a, &b);
    assert!(matches!(
        feature(&assessment, Feature::Surname).outcome,
        Outcome::Partial(_)
    ));
    assert_eq!(assessment.cultures.len(), 1);
}

#[test]
fn haugen_and_haug_are_partial_and_never_held_against_the_pair() {
    let a = person("Ole", "Haugen", Sex::Male, born(on(1850, 3, 4), Some("Norge")));
    let b = person("Ole", "Haug", Sex::Male, born(on(1850, 3, 4), Some("Norge")));
    let surname = feature(&assess(&a, &b), Feature::Surname).clone();
    assert!(matches!(surname.outcome, Outcome::Partial(_)), "{surname:?}");
    assert!(surname.weight > 0.0);
}

#[test]
fn a_farm_name_changed_after_a_move_is_missing_evidence_not_a_disagreement() {
    let a = person("Ole", "Haugen", Sex::Male, born(on(1850, 3, 4), Some("Norge")));
    let b = person("Ole", "Brekke", Sex::Male, born(on(1850, 3, 4), Some("Norge")));
    let surname = feature(&assess(&a, &b), Feature::Surname).clone();
    assert_eq!(surname.outcome, Outcome::Missing);
    assert!(surname.weight.abs() < f64::EPSILON);
}

#[test]
fn an_english_surname_mismatch_disagrees() {
    let a = person("John", "Smith", Sex::Male, born(on(1850, 3, 4), Some("England")));
    let b = person("John", "Brown", Sex::Male, born(on(1850, 3, 4), Some("England")));
    assert_eq!(feature(&assess(&a, &b), Feature::Surname).outcome, Outcome::Disagree);
}

#[test]
fn census_1852_and_baptism_1849_agree_within_census_tolerance() {
    let census = vec![vital(
        VitalKind::Birth,
        point(1852, None, None),
        DateBasis::FromAge,
        Some("Norge"),
    )];
    let baptism = vec![vital(
        VitalKind::Baptism,
        on(1849, 6, 10),
        DateBasis::Recorded,
        Some("Norge"),
    )];
    let a = person("Ole", "Olsen", Sex::Male, census);
    let b = person("Ole", "Olsen", Sex::Male, baptism);
    let assessment = assess(&a, &b);
    let birth = feature(&assessment, Feature::Birth);
    assert!(matches!(birth.outcome, Outcome::Partial(_)), "{birth:?}");
    assert!(birth.weight > 2.0, "{birth:?}");
    assert!(assessment.band >= MatchBand::Probable, "{assessment:#?}");
}

#[test]
fn a_census_age_off_by_more_than_ten_years_disagrees() {
    let census = vec![vital(
        VitalKind::Birth,
        point(1862, None, None),
        DateBasis::FromAge,
        None,
    )];
    let a = person("Ole", "Olsen", Sex::Male, census);
    let b = person("Ole", "Olsen", Sex::Male, born(on(1849, 6, 10), None));
    assert_eq!(feature(&assess(&a, &b), Feature::Birth).outcome, Outcome::Disagree);
}

#[test]
fn a_birth_three_years_off_stays_possible() {
    for country in [None, Some("Norge")] {
        let a = person("Ole", "Olsen", Sex::Male, born(on(1850, 5, 1), country));
        let b = person("Ole", "Olsen", Sex::Male, born(on(1853, 5, 1), country));
        let assessment = assess(&a, &b);
        assert_eq!(assessment.band, MatchBand::Possible, "{assessment:#?}");
        assert!(matches!(
            feature(&assessment, Feature::Birth).outcome,
            Outcome::Partial(_)
        ));
    }
}

#[test]
fn a_clerical_slip_in_an_exact_date_scores_as_a_near_agreement() {
    let near = |x: GenealogicalDate, y: GenealogicalDate| {
        let a = person("Ole", "Olsen", Sex::Male, born(x, None));
        let b = person("Ole", "Olsen", Sex::Male, born(y, None));
        feature(&assess(&a, &b), Feature::Birth).outcome
    };
    let julian = near(on(1750, 4, 1), on(1750, 4, 12));
    assert!(matches!(julian, Outcome::Partial(s) if s >= 0.97), "{julian:?}");
    let transposed = near(on(1850, 3, 5), on(1850, 5, 3));
    assert!(matches!(transposed, Outcome::Partial(s) if s >= 0.97), "{transposed:?}");
    let plain = near(on(1850, 3, 5), on(1850, 5, 4));
    assert!(matches!(plain, Outcome::Partial(s) if s < 0.9), "{plain:?}");
}

#[test]
fn a_different_asserted_sex_caps_the_score() {
    let a = person("Kari", "Olsen", Sex::Male, born(on(1850, 3, 4), None));
    let b = person("Kari", "Olsen", Sex::Female, born(on(1850, 3, 4), None));
    let assessment = assess(&a, &b);
    assert_eq!(feature(&assessment, Feature::Sex).outcome, Outcome::Conflict);
    assert_eq!(assessment.band, MatchBand::Unlikely);
    assert!(assessment.score <= 0.01);
}

#[test]
fn an_unknown_sex_is_missing() {
    let a = person("Kari", "Olsen", Sex::Unknown, Vec::new());
    let b = person("Kari", "Olsen", Sex::Female, Vec::new());
    let assessment = assess(&a, &b);
    let sex = feature(&assessment, Feature::Sex);
    assert_eq!(sex.outcome, Outcome::Missing);
    assert!(sex.weight.abs() < f64::EPSILON, "missing evidence adds nothing");
    assert!(feature(&assessment, Feature::Birth).weight.abs() < f64::EPSILON);
}

#[test]
fn a_death_before_the_others_birth_is_a_conflict() {
    let mut a = person("Ole", "Olsen", Sex::Male, Vec::new());
    a.vitals
        .push(vital(VitalKind::Death, on(1840, 1, 1), DateBasis::Recorded, None));
    let b = person("Ole", "Olsen", Sex::Male, born(on(1850, 3, 4), None));
    let assessment = assess(&a, &b);
    assert_eq!(feature(&assessment, Feature::Lifespan).outcome, Outcome::Conflict);
    assert_eq!(assessment.band, MatchBand::Unlikely);
    assert_eq!(assess(&b, &a).band, MatchBand::Unlikely);
}

#[test]
fn a_burial_stands_in_for_death() {
    let mut a = person("Ole", "Olsen", Sex::Male, Vec::new());
    a.vitals
        .push(vital(VitalKind::Death, on(1900, 1, 1), DateBasis::Recorded, None));
    let mut b = person("Ole", "Olsen", Sex::Male, Vec::new());
    b.vitals
        .push(vital(VitalKind::Burial, on(1900, 1, 12), DateBasis::Recorded, None));
    assert_eq!(feature(&assess(&a, &b), Feature::Death).outcome, Outcome::Agree);
}

#[test]
fn a_shared_record_origin_is_deterministic() {
    let origin = RecordOrigin {
        dataset: DatasetId::global("digitalarkivet"),
        record: "pf01036389002345".to_owned(),
        item: None,
        digest: None,
        run: ImportRunId::from_uuid(Uuid::now_v7()),
    };
    let mut a = person("Ole", "Olsen", Sex::Male, Vec::new());
    let mut b = person("Ola", "Olsen", Sex::Male, Vec::new());
    a.origins.push(origin.clone());
    b.origins.push(RecordOrigin {
        run: ImportRunId::from_uuid(Uuid::now_v7()),
        ..origin
    });
    assert_eq!(assess(&a, &b).band, MatchBand::Deterministic);
}

#[test]
fn a_shared_external_id_is_deterministic_and_a_high_score_is_not() {
    let external = ExternalId {
        authority: "FamilySearch".to_owned(),
        value: "KWZ1-234".to_owned(),
        kind: None,
        url: None,
    };
    let mut a = person("Ole", "Olsen", Sex::Male, born(on(1850, 3, 4), None));
    let b = a.clone();
    assert_eq!(
        assess(&a, &b).band,
        MatchBand::Probable,
        "an exact copy is still only a score"
    );
    let mut c = person("Per", "Hansen", Sex::Male, Vec::new());
    a.external_ids.push(external.clone());
    c.external_ids.push(external);
    assert_eq!(assess(&a, &c).band, MatchBand::Deterministic);
}

#[test]
fn a_toy_pack_loaded_as_data_changes_the_score() {
    let speaker = |given: &str| {
        let mut profile = person(given, "", Sex::Male, Vec::new());
        profile.names[0].language = Some(LanguageTag::new("tq"));
        profile
    };
    let (a, b) = (speaker("Zorbo"), speaker("Quimble"));
    let settings = MatchSettings::default();
    let before = assess_persons(&a, &b, &DATA, &settings);
    let toy = PackSource::new("toy.toml", include_str!("../../tests/fixtures/matching/toy.toml"));
    let data = DATA.clone().layered([toy], None).unwrap();
    let after = assess_persons(&a, &b, &data, &settings);
    assert_eq!(feature(&before, Feature::GivenName).outcome, Outcome::Disagree);
    assert!(matches!(
        feature(&after, Feature::GivenName).outcome,
        Outcome::Partial(_)
    ));
    assert!(after.score > before.score);
    assert!(after.cultures.iter().any(|c| c.as_str() == "toy"));
}

#[test]
fn every_score_is_bounded_and_the_band_follows_the_thresholds() {
    let a = person("Ole", "Olsen", Sex::Male, born(on(1850, 3, 4), None));
    let strict = MatchSettings {
        probable: 1.0,
        possible: 1.0,
        ..MatchSettings::default()
    };
    assert_eq!(assess_persons(&a, &a, &DATA, &strict).band, MatchBand::Unlikely);
    let lax = MatchSettings {
        probable: 0.0,
        possible: 0.0,
        ..MatchSettings::default()
    };
    let b = person("Per", "Hansen", Sex::Female, Vec::new());
    assert_eq!(assess_persons(&a, &b, &DATA, &lax).band, MatchBand::Probable);
}

fn given_name() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "",
        "Ole",
        "Ola",
        "Peder",
        "Per",
        "Guldbrand",
        "Gulbrand",
        "Anne",
        "Kari",
        "Bill",
    ])
}

fn surname() -> impl Strategy<Value = &'static str> {
    prop::sample::select(vec![
        "",
        "Olsen",
        "Olsøn",
        "Olsdatter",
        "Haugen",
        "Haug",
        "Smith",
        "Brekke",
    ])
}

fn sex() -> impl Strategy<Value = Sex> {
    prop_oneof![Just(Sex::Male), Just(Sex::Female), Just(Sex::Unknown)]
}

fn country() -> impl Strategy<Value = Option<&'static str>> {
    prop::sample::select(vec![None, Some("Norge"), Some("England"), Some("Danmark")])
}

fn basis() -> impl Strategy<Value = DateBasis> {
    prop_oneof![Just(DateBasis::Recorded), Just(DateBasis::FromAge)]
}

fn a_date() -> impl Strategy<Value = GenealogicalDate> {
    (1690i32..1910, prop::option::of((1u8..=12, prop::option::of(1u8..=28))))
        .prop_map(|(year, rest)| point(year, rest.map(|(m, _)| m), rest.and_then(|(_, d)| d)))
}

fn profile() -> impl Strategy<Value = PersonProfile> {
    let birth = prop::option::of((a_date(), basis(), prop::bool::ANY));
    let lifespan = prop::option::of(0i32..95);
    (given_name(), surname(), sex(), country(), birth, lifespan).prop_map(
        |(given, surname, sex, country, birth, lifespan)| {
            let mut vitals = Vec::new();
            if let Some((date, basis, baptism)) = birth {
                let kind = if baptism { VitalKind::Baptism } else { VitalKind::Birth };
                if let (Some(years), Some(year)) = (lifespan, crate::matching::date::year(&date)) {
                    vitals.push(vital(
                        VitalKind::Death,
                        point(year + years + 1, None, None),
                        DateBasis::Recorded,
                        country,
                    ));
                }
                vitals.push(vital(kind, date, basis, country));
            }
            person(given, surname, sex, vitals)
        },
    )
}

proptest! {
    #[test]
    fn assessment_is_symmetric(a in profile(), b in profile()) {
        let (ab, ba) = (assess(&a, &b), assess(&b, &a));
        prop_assert!((ab.score - ba.score).abs() < 1e-12, "{} vs {}", ab.score, ba.score);
        prop_assert_eq!(ab.band, ba.band);
        prop_assert_eq!(ab.features.len(), ba.features.len());
        for (x, y) in ab.features.iter().zip(&ba.features) {
            prop_assert_eq!(x.outcome, y.outcome);
            prop_assert!((x.weight - y.weight).abs() < 1e-12);
        }
        prop_assert_eq!(ab.cultures, ba.cultures);
    }

    #[test]
    fn a_score_is_a_probability(a in profile(), b in profile()) {
        let score = assess(&a, &b).score;
        prop_assert!((0.0..=1.0).contains(&score), "{score}");
    }

    #[test]
    fn a_record_matches_itself_at_least_as_well_as_any_other(a in profile(), b in profile()) {
        let (itself, other) = (assess(&a, &a), assess(&a, &b));
        prop_assert!(itself.score >= other.score, "self {} < other {}", itself.score, other.score);
    }
}
