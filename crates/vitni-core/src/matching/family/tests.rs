//! The family assessment's table cases (ADR 0038 §2, §4): one marriage from a church book and from a
//! GEDCOM file.

use proptest::prelude::{Strategy, prop, prop_assert, proptest};
use uuid::Uuid;

use crate::enums::Sex;
use crate::ids::ImportRunId;
use crate::matching::profile::{FamilyProfile, PersonProfile, VitalKind};
use crate::matching::tests::{
    DATA, a_date, feature, given_name, marriage, on, parish, person, point, profile, relative, vital,
};
use crate::matching::weights;
use crate::matching::{
    DateBasis, Feature, FeatureValue, MatchAssessment, MatchBand, MatchSettings, Outcome, assess_families,
};
use crate::origin::{DatasetId, RecordOrigin};
use crate::text::ExternalId;

fn assess(a: &FamilyProfile, b: &FamilyProfile) -> MatchAssessment {
    assess_families(a, b, &DATA, &MatchSettings::default())
}

fn origin(dataset: &str, record: &str) -> RecordOrigin {
    RecordOrigin {
        dataset: DatasetId::global(dataset),
        record: record.to_owned(),
        item: None,
        digest: None,
        run: ImportRunId::from_uuid(Uuid::now_v7()),
    }
}

/// A partner as a church book's marriage entry states them: a name, an age, and a father.
fn entered(given: &str, surname: &str, sex: Sex, (born, father): (i32, &str)) -> PersonProfile {
    let birth = vital(VitalKind::Birth, point(born, None, None), DateBasis::FromAge, None);
    PersonProfile {
        parents: vec![relative(father, "", Sex::Male, None)],
        ..person(given, surname, sex, vec![birth])
    }
}

/// A partner as a GEDCOM file carries them: a name and an exact birth in the parish.
fn carried(given: &str, surname: &str, sex: Sex, born: (i32, u8, u8)) -> PersonProfile {
    let mut birth = vital(VitalKind::Birth, on(born.0, born.1, born.2), DateBasis::Recorded, None);
    birth.place = Some(parish("Ringsaker"));
    person(given, surname, sex, vec![birth])
}

/// The Ringsaker church book's entry for the 1877 marriage of Guldbrand and Marte.
fn church_book() -> FamilyProfile {
    FamilyProfile {
        partners: vec![
            entered("Guldbrand", "Olsen", Sex::Male, (1850, "Ole")),
            entered("Marte", "Pedersdtr.", Sex::Female, (1853, "Peder")),
        ],
        children: Vec::new(),
        marriage: Some(marriage(("Guldbrand", "Olsen"), ("Marte", "Pedersdtr."))),
        origins: vec![origin("digitalarkivet", "vi01036389000412")],
        external_ids: Vec::new(),
    }
}

/// The same marriage in someone's GEDCOM file, spelled their way.
fn gedcom() -> FamilyProfile {
    FamilyProfile {
        partners: vec![
            carried("Gulbrand", "Olsøn", Sex::Male, (1850, 3, 4)),
            carried("Marthe", "Pedersdatter", Sex::Female, (1853, 5, 2)),
        ],
        children: Vec::new(),
        marriage: Some(marriage(("Gulbrand", "Olsøn"), ("Marthe", "Pedersdatter"))),
        origins: vec![origin("gedcom:3f2a", "@F12@")],
        external_ids: Vec::new(),
    }
}

fn partners(assessment: &MatchAssessment) -> Vec<(Outcome, f64)> {
    let mut found = Vec::new();
    for f in &assessment.features {
        if f.feature == Feature::Partner {
            found.push((f.outcome, f.weight));
        }
    }
    found
}

#[test]
fn one_marriage_from_a_church_book_and_a_gedcom_file_is_probable() {
    let assessment = assess(&church_book(), &gedcom());
    assert_eq!(assessment.band, MatchBand::Probable, "{assessment:#?}");
    let partners = partners(&assessment);
    assert_eq!(partners.len(), 2, "{assessment:#?}");
    assert!(
        partners
            .iter()
            .all(|(outcome, weight)| *outcome == Outcome::Agree && *weight > 0.0),
        "{partners:?}"
    );
    assert_eq!(assessment.parts.len(), 2);
    assert!(assessment.parts.iter().all(|part| part.band == MatchBand::Probable));
    assert_eq!(feature(&assessment, Feature::Marriage).outcome, Outcome::Agree);
    assert_eq!(feature(&assessment, Feature::MarriagePlace).outcome, Outcome::Agree);
    let patronymic = feature(&assessment.parts[0], Feature::Patronymic);
    assert_eq!(patronymic.outcome, Outcome::Agree, "Olsøn ↔ father Ole: {patronymic:?}");
}

#[test]
fn a_family_recording_only_its_husband_is_not_paired_against_one_recording_only_its_wife() {
    let mut husband_only = church_book();
    husband_only.partners.truncate(1);
    let mut wife_only = gedcom();
    wife_only.partners.remove(0);
    let assessment = assess(&husband_only, &wife_only);
    assert_eq!(partners(&assessment), [(Outcome::Missing, 0.0)], "{assessment:#?}");
    assert!(assessment.parts.is_empty());
    assert!(
        assessment.features.iter().all(|f| f.outcome != Outcome::Conflict),
        "{assessment:#?}"
    );
}

#[test]
fn a_partner_term_weighs_what_its_part_weighs_up_to_the_support_cap() {
    let assessment = assess(&church_book(), &gedcom());
    let mut remarried = gedcom();
    remarried.partners[1] = carried("Ingeborg", "Hansdatter", Sex::Female, (1860, 1, 9));
    let apart = assess(&church_book(), &remarried);
    for assessment in [assessment, apart] {
        for ((_, weight), part) in partners(&assessment).iter().zip(&assessment.parts) {
            let summed: f64 = part.features.iter().map(|f| f.weight).sum();
            let expected = summed.min(weights::PARTNER_SUPPORT);
            assert!((weight - expected).abs() < 1e-9, "{weight} against {summed}");
        }
    }
}

#[test]
fn a_remarriage_without_a_marriage_date_is_still_unlikely() {
    let mut remarried = gedcom();
    remarried.partners[1] = carried("Ingeborg", "Hansdatter", Sex::Female, (1860, 1, 9));
    remarried.marriage = None;
    let assessment = assess(&church_book(), &remarried);
    assert_eq!(assessment.band, MatchBand::Unlikely, "{assessment:#?}");
}

#[test]
fn partners_pair_by_who_they_are_not_by_their_order() {
    let mut reordered = gedcom();
    reordered.partners.reverse();
    let assessment = assess(&church_book(), &reordered);
    assert_eq!(assessment.band, MatchBand::Probable, "{assessment:#?}");
    let paired: Vec<(Option<FeatureValue>, Option<FeatureValue>)> = assessment
        .parts
        .iter()
        .map(|part| {
            let given = feature(part, Feature::GivenName);
            (given.left.clone(), given.right.clone())
        })
        .collect();
    let name = |text: &str| Some(FeatureValue::Name(text.to_owned()));
    assert!(paired.contains(&(name("Guldbrand"), name("Gulbrand"))), "{paired:?}");
    assert!(paired.contains(&(name("Marte"), name("Marthe"))), "{paired:?}");
}

#[test]
fn a_different_bride_is_a_different_marriage() {
    let mut remarried = gedcom();
    remarried.partners[1] = carried("Ingeborg", "Hansdatter", Sex::Female, (1860, 1, 9));
    remarried.marriage = Some(marriage(("Gulbrand", "Olsøn"), ("Ingeborg", "Hansdatter")));
    if let Some(event) = &mut remarried.marriage {
        event.date = Some(on(1889, 6, 2));
    }
    let assessment = assess(&church_book(), &remarried);
    assert!(assessment.band < MatchBand::Possible, "{assessment:#?}");
    assert!(
        partners(&assessment)
            .iter()
            .any(|(outcome, _)| *outcome == Outcome::Disagree),
        "{assessment:#?}"
    );
}

#[test]
fn a_family_with_one_partner_known_leaves_the_other_missing() {
    let mut widow = gedcom();
    widow.partners.remove(0);
    let assessment = assess(&church_book(), &widow);
    let terms = partners(&assessment);
    assert_eq!(terms.len(), 1, "{assessment:#?}");
    assert_eq!(assessment.parts.len(), 1);
}

#[test]
fn a_shared_child_supports_the_pair() {
    let with_child = |mut family: FamilyProfile| {
        family
            .children
            .push(relative("Anne", "Gulbrandsdatter", Sex::Female, Some(on(1878, 8, 1))));
        family
    };
    let assessment = assess(&with_child(church_book()), &with_child(gedcom()));
    let children = feature(&assessment, Feature::Children);
    assert_eq!(children.outcome, Outcome::Agree, "{children:?}");
    assert!(children.weight > 0.0);
    assert_eq!(
        feature(&assess(&church_book(), &gedcom()), Feature::Children).outcome,
        Outcome::Missing
    );
}

#[test]
fn a_shared_external_id_is_deterministic() {
    let external = ExternalId {
        authority: "FamilySearch".to_owned(),
        value: "K2M3-9PQ".to_owned(),
        kind: None,
        url: None,
    };
    let (mut a, mut b) = (church_book(), gedcom());
    a.external_ids.push(external.clone());
    b.external_ids.push(external);
    assert_eq!(assess(&a, &b).band, MatchBand::Deterministic);
}

fn family() -> impl Strategy<Value = FamilyProfile> {
    (
        prop::collection::vec(profile(), 0..3),
        prop::option::of(a_date()),
        prop::option::of(given_name()),
    )
        .prop_map(|(partners, married, child)| {
            let mut wedding = marriage(("", ""), ("", ""));
            wedding.date = married;
            FamilyProfile {
                partners,
                children: child
                    .map(|given| relative(given, "", Sex::Female, None))
                    .into_iter()
                    .collect(),
                marriage: Some(wedding),
                origins: Vec::new(),
                external_ids: Vec::new(),
            }
        })
}

proptest! {
    #[test]
    fn a_family_score_is_a_probability(a in family(), b in family()) {
        let score = assess(&a, &b).score;
        prop_assert!((0.0..=1.0).contains(&score), "{score}");
    }

    #[test]
    fn a_family_matches_itself_at_least_as_well_as_any_other(a in family(), b in family()) {
        let (itself, other) = (assess(&a, &a), assess(&a, &b));
        prop_assert!(itself.score >= other.score, "self {} < other {}", itself.score, other.score);
    }
}
