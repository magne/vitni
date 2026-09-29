//! The assessment's table cases and properties (ADR 0038 §3–§6).

use std::sync::LazyLock;

use proptest::prelude::{Just, Strategy, prop, prop_assert, prop_assert_eq, prop_oneof, proptest};
use uuid::Uuid;

use crate::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
use crate::enums::Sex;
use crate::ids::ImportRunId;
use crate::matching::pack::PackSource;
use crate::matching::profile::{PersonProfile, PlaceProfile, Relative, VitalEvent, VitalKind};
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

fn relative(given: &str, surname: &str, sex: Sex, birth: Option<GenealogicalDate>) -> Relative {
    Relative {
        names: vec![name(given, surname)],
        sex: Some(sex),
        birth: birth.map(|date| vital(VitalKind::Birth, date, DateBasis::Recorded, None)),
    }
}

fn ole_olsen() -> PersonProfile {
    person("Ole", "Olsen", Sex::Male, born(on(1850, 3, 4), Some("Norge")))
}

fn with_parents(mut profile: PersonProfile, parents: Vec<Relative>) -> PersonProfile {
    profile.parents = parents;
    profile
}

#[test]
fn two_ole_olsens_born_the_same_year_separate_on_their_fathers() {
    let (a, b) = (ole_olsen(), ole_olsen());
    assert_eq!(
        assess(&a, &b).band,
        MatchBand::Probable,
        "with no parents they look alike"
    );

    let father = relative("Ole", "Hansen", Sex::Male, Some(on(1815, 5, 1)));
    let same = assess(
        &with_parents(a.clone(), vec![father.clone()]),
        &with_parents(b.clone(), vec![father.clone()]),
    );
    assert_eq!(same.band, MatchBand::Probable, "{same:#?}");
    assert_eq!(feature(&same, Feature::Father).outcome, Outcome::Agree);

    let other = relative("Ole", "Nilsen", Sex::Male, Some(on(1830, 9, 12)));
    let apart = assess(
        &with_parents(a.clone(), vec![father.clone()]),
        &with_parents(b.clone(), vec![other.clone()]),
    );
    assert_eq!(feature(&apart, Feature::Father).outcome, Outcome::Disagree);
    assert!(apart.band < MatchBand::Probable, "{apart:#?}");
    assert!(apart.score < same.score);

    let mothers = |given: &str, year: i32| relative(given, "Olsdatter", Sex::Female, Some(on(year, 2, 2)));
    let both = assess(
        &with_parents(a, vec![father, mothers("Marte", 1820)]),
        &with_parents(b, vec![other, mothers("Kari", 1826)]),
    );
    assert_eq!(feature(&both, Feature::Mother).outcome, Outcome::Disagree);
    assert_eq!(both.band, MatchBand::Unlikely, "{both:#?}");
}

#[test]
fn fathers_of_different_given_names_disagree() {
    let child = || person("John", "Smith", Sex::Male, born(on(1850, 3, 4), Some("England")));
    let a = with_parents(child(), vec![relative("William", "Smith", Sex::Male, None)]);
    let b = with_parents(child(), vec![relative("George", "Smith", Sex::Male, None)]);
    let father = feature(&assess(&a, &b), Feature::Father).clone();
    assert_eq!(father.outcome, Outcome::Disagree, "{father:?}");
}

#[test]
fn a_fathers_name_similarity_is_scaled_from_the_name_floor() {
    let father_weight = |left: &str, right: &str| {
        let a = with_parents(ole_olsen(), vec![relative(left, "Hansen", Sex::Male, None)]);
        let b = with_parents(ole_olsen(), vec![relative(right, "Hansen", Sex::Male, None)]);
        feature(&assess(&a, &b), Feature::Father).weight
    };
    let same = father_weight("Anders", "Anders");
    let similar = father_weight("Anders", "Andreas");
    assert!(similar < same / 2.0, "{similar} against {same}");
}

#[test]
fn an_undated_father_of_the_same_name_is_partial_support() {
    let father = || relative("Ole", "Hansen", Sex::Male, None);
    let a = with_parents(ole_olsen(), vec![father()]);
    let b = with_parents(ole_olsen(), vec![father()]);
    let assessment = assess(&a, &b);
    let father = feature(&assessment, Feature::Father);
    assert!(matches!(father.outcome, Outcome::Partial(_)), "{father:?}");
    assert!(father.weight > 0.0);
    assert_eq!(feature(&assessment, Feature::Mother).outcome, Outcome::Missing);
}

#[test]
fn a_parent_of_unknown_sex_is_neither_father_nor_mother() {
    let parent = || relative("Ole", "Hansen", Sex::Unknown, Some(on(1815, 5, 1)));
    let a = with_parents(ole_olsen(), vec![parent()]);
    let b = with_parents(ole_olsen(), vec![parent()]);
    let assessment = assess(&a, &b);
    assert_eq!(feature(&assessment, Feature::Father).outcome, Outcome::Missing);
    assert_eq!(feature(&assessment, Feature::Mother).outcome, Outcome::Missing);
}

#[test]
fn a_patronymic_is_checked_against_the_candidate_father() {
    let census = ole_olsen();
    let patronymic = |father: &str| {
        let b = with_parents(ole_olsen(), vec![relative(father, "Hansen", Sex::Male, None)]);
        feature(&assess(&census, &b), Feature::Patronymic).clone()
    };
    assert_eq!(patronymic("Ole").outcome, Outcome::Agree);
    assert_eq!(patronymic("Olav").outcome, Outcome::Agree, "Olav is in Ole's class");
    let hans = patronymic("Hans");
    assert_eq!(hans.outcome, Outcome::Disagree, "{hans:?}");
    assert!(hans.weight < 0.0);
}

#[test]
fn patronymic_stems_meet_their_fathers_names() {
    for (surname, father) in [
        ("Hansen", "Hans"),
        ("Olsdatter", "Ole"),
        ("Pedersen", "Peder"),
        ("Johannesen", "Johannes"),
        ("Andreassen", "Andreas"),
        ("Rasmussen", "Rasmus"),
        ("Knudsen", "Knut"),
    ] {
        let a = person("Anne", surname, Sex::Female, born(on(1850, 3, 4), Some("Norge")));
        let b = with_parents(
            person("Anne", surname, Sex::Female, born(on(1850, 3, 4), Some("Norge"))),
            vec![relative(father, "", Sex::Male, None)],
        );
        let outcome = feature(&assess(&a, &b), Feature::Patronymic).outcome;
        assert_eq!(outcome, Outcome::Agree, "{surname} ↔ {father}");
    }
}

#[test]
fn a_patronymic_is_not_checked_where_both_sides_state_a_father_or_no_culture_is_patronymic() {
    let father = || relative("Hans", "Olsen", Sex::Male, None);
    let a = with_parents(ole_olsen(), vec![father()]);
    let b = with_parents(ole_olsen(), vec![father()]);
    assert_eq!(feature(&assess(&a, &b), Feature::Patronymic).outcome, Outcome::Missing);
    let english = person("John", "Johnson", Sex::Male, born(on(1850, 3, 4), Some("England")));
    let with_father = with_parents(english.clone(), vec![relative("William", "Johnson", Sex::Male, None)]);
    assert_eq!(
        feature(&assess(&english, &with_father), Feature::Patronymic).outcome,
        Outcome::Missing
    );
}

#[test]
fn a_shared_child_supports_the_pair_and_disjoint_children_are_no_evidence() {
    let with_children = |children: Vec<Relative>| PersonProfile {
        children,
        ..ole_olsen()
    };
    let anne = || relative("Anne", "Olsdatter", Sex::Female, Some(on(1880, 1, 9)));
    let kari = || relative("Kari", "Olsdatter", Sex::Female, Some(on(1884, 7, 1)));
    let shared = assess(&with_children(vec![anne(), kari()]), &with_children(vec![kari()]));
    let children = feature(&shared, Feature::Children);
    assert_eq!(children.outcome, Outcome::Agree, "{children:?}");
    assert!(children.weight > 0.0);
    let disjoint = assess(&with_children(vec![anne()]), &with_children(vec![kari()]));
    let children = feature(&disjoint, Feature::Children);
    assert_eq!(children.outcome, Outcome::Missing, "{children:?}");
    assert!(children.weight.abs() < f64::EPSILON);
}

#[test]
fn a_shared_partner_supports_the_pair() {
    let with_partner = |given: &str| PersonProfile {
        partners: vec![relative(given, "Hansdatter", Sex::Female, Some(on(1852, 4, 4)))],
        ..ole_olsen()
    };
    let assessment = assess(&with_partner("Marte"), &with_partner("Martha"));
    assert!(feature(&assessment, Feature::Partners).weight > 0.0);
    assert_eq!(
        feature(
            &assess(&with_partner("Marte"), &with_partner("Ingeborg")),
            Feature::Partners
        )
        .outcome,
        Outcome::Missing
    );
}

#[test]
fn an_occupation_in_common_is_weak_support_and_a_different_one_is_no_evidence() {
    let working = |occupation: &str| PersonProfile {
        occupations: vec![occupation.to_owned()],
        ..ole_olsen()
    };
    let same = feature(&assess(&working("Husmann"), &working("husmann")), Feature::Occupation).clone();
    assert_eq!(same.outcome, Outcome::Agree);
    assert!(same.weight > 0.0);
    let other = feature(&assess(&working("Husmann"), &working("Skomaker")), Feature::Occupation).clone();
    assert_eq!(other.outcome, Outcome::Missing);
}

fn household(item: Option<&str>) -> RecordOrigin {
    RecordOrigin {
        dataset: DatasetId::global("digitalarkivet"),
        record: "bf01036389000123".to_owned(),
        item: item.map(ToOwned::to_owned),
        digest: None,
        run: ImportRunId::from_uuid(Uuid::now_v7()),
    }
}

#[test]
fn two_items_of_one_record_are_different_people() {
    let (mut a, mut b) = (ole_olsen(), ole_olsen());
    a.origins.push(household(Some("person:1")));
    b.origins.push(household(Some("person:2")));
    let assessment = assess(&a, &b);
    assert_eq!(feature(&assessment, Feature::Record).outcome, Outcome::Conflict);
    assert_eq!(assessment.band, MatchBand::Unlikely);
    assert!(assessment.score <= 0.01);

    b.origins = vec![household(Some("person:1"))];
    let same = assess(&a, &b);
    assert_eq!(same.band, MatchBand::Deterministic);
    assert!(same.features.iter().all(|f| f.feature != Feature::Record));
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
    let father = prop::option::of((given_name(), prop::option::of(a_date())));
    (given_name(), surname(), sex(), country(), birth, lifespan, father).prop_map(
        |(given, surname, sex, country, birth, lifespan, father)| {
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
            let mut profile = person(given, surname, sex, vitals);
            profile
                .parents
                .extend(father.map(|(given, born)| relative(given, "", Sex::Male, born)));
            profile
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
