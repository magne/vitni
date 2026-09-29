//! The event assessment's table cases (ADR 0038 §2, §4): the marriage and census cases.

use proptest::prelude::{Strategy, prop, prop_assert, prop_assert_eq, proptest};

use crate::enums::{EventType, ParticipantRole, Sex};
use crate::matching::profile::EventProfile;
use crate::matching::tests::{DATA, a_date, feature, given_name, household, marriage, on, sex, taking_part};
use crate::matching::{CultureId, Feature, MatchAssessment, MatchBand, MatchSettings, Outcome, assess_events};

fn assess(a: &EventProfile, b: &EventProfile) -> MatchAssessment {
    assess_events(a, b, &DATA, &MatchSettings::default())
}

/// The same marriage as a GEDCOM file carries it: both partners the event's primary participants.
fn as_gedcom(mut event: EventProfile) -> EventProfile {
    for participant in &mut event.participants {
        participant.role = ParticipantRole::Primary;
    }
    event
}

#[test]
fn one_marriage_written_two_ways_is_probable() {
    let church = marriage(("Guldbrand", "Olsen"), ("Marte", "Pedersdtr."));
    let gedcom = as_gedcom(marriage(("Gulbrand", "Olsøn"), ("Marthe", "Pedersdatter")));
    let assessment = assess(&church, &gedcom);
    assert_eq!(assessment.band, MatchBand::Probable, "{assessment:#?}");
    assert_eq!(feature(&assessment, Feature::EventType).outcome, Outcome::Agree);
    assert_eq!(feature(&assessment, Feature::Date).outcome, Outcome::Agree);
    assert_eq!(feature(&assessment, Feature::Place).outcome, Outcome::Agree);
    let principals: Vec<_> = assessment
        .features
        .iter()
        .filter(|f| f.feature == Feature::Principal)
        .collect();
    assert_eq!(principals.len(), 2, "{principals:#?}");
    assert!(principals.iter().all(|f| f.weight > 0.0), "{principals:#?}");
    let cultures: Vec<&str> = assessment.cultures.iter().map(CultureId::as_str).collect();
    assert_eq!(cultures, ["universal", "da", "no"]);
    assert!(assessment.parts.is_empty());
}

#[test]
fn a_marriage_with_another_groom_is_another_marriage() {
    let church = marriage(("Guldbrand", "Olsen"), ("Marte", "Pedersdatter"));
    let other = marriage(("Hans", "Nilsen"), ("Marte", "Pedersdatter"));
    let assessment = assess(&church, &other);
    let disagreeing = assessment
        .features
        .iter()
        .filter(|f| f.feature == Feature::Principal && f.outcome == Outcome::Disagree)
        .count();
    assert_eq!(disagreeing, 1, "{assessment:#?}");
    assert!(assessment.band < MatchBand::Probable, "{assessment:#?}");
}

#[test]
fn principals_of_different_sexes_are_not_paired() {
    let church = marriage(("Ole", "Olsen"), ("Ola", "Olsdatter"));
    let mut swapped = church.clone();
    swapped.participants.reverse();
    for participant in &mut swapped.participants {
        participant.person.sex = participant.person.sex.clone().map(|sex| match sex {
            Sex::Male => Sex::Female,
            Sex::Female | Sex::Unknown | Sex::Intersex | Sex::Other(_) => Sex::Male,
        });
    }
    let assessment = assess(&church, &swapped);
    for principal in assessment.features.iter().filter(|f| f.feature == Feature::Principal) {
        assert_ne!(principal.left, principal.right, "paired across sexes: {principal:?}");
    }
}

#[test]
fn a_different_event_type_is_a_conflict_and_a_christening_is_a_baptism() {
    let church = marriage(("Ole", "Olsen"), ("Kari", "Hansdatter"));
    let baptism = EventProfile {
        event_type: Some(EventType::Baptism),
        ..church.clone()
    };
    let assessment = assess(&church, &baptism);
    assert_eq!(feature(&assessment, Feature::EventType).outcome, Outcome::Conflict);
    assert_eq!(assessment.band, MatchBand::Unlikely);
    let christening = EventProfile {
        event_type: Some(EventType::Christening),
        ..church
    };
    let assessment = assess(&baptism, &christening);
    assert_eq!(feature(&assessment, Feature::EventType).outcome, Outcome::Agree);
}

#[test]
fn a_custom_type_compares_case_folded_and_otherwise_is_no_evidence() {
    let typed = |event_type: EventType| EventProfile {
        event_type: Some(event_type),
        ..marriage(("Ole", "Olsen"), ("Kari", "Hansdatter"))
    };
    let custom = |text: &str| typed(EventType::Custom(text.to_owned()));
    let same = assess(&custom("Vielse"), &custom("vielse"));
    assert_eq!(feature(&same, Feature::EventType).outcome, Outcome::Agree);
    let other = assess(&custom("Vielse"), &typed(EventType::Marriage));
    let event_type = feature(&other, Feature::EventType);
    assert_eq!(event_type.outcome, Outcome::Missing);
    assert!(event_type.weight.abs() < f64::EPSILON);
}

#[test]
fn a_shared_witness_supports_the_pair_and_different_witnesses_are_no_evidence() {
    let witnessed = |given: &str| {
        let mut event = marriage(("Ole", "Olsen"), ("Kari", "Hansdatter"));
        event
            .participants
            .push(taking_part(ParticipantRole::Witness, given, "Haugen", Sex::Male));
        event
    };
    let shared = assess(&witnessed("Anders"), &witnessed("Anders"));
    let participants = feature(&shared, Feature::Participants);
    assert!(participants.weight > 0.0, "{participants:?}");
    let different = assess(&witnessed("Anders"), &witnessed("Torstein"));
    let participants = feature(&different, Feature::Participants);
    assert_eq!(participants.outcome, Outcome::Missing, "{participants:?}");
}

#[test]
fn a_marriage_a_year_later_is_only_partial() {
    let church = marriage(("Ole", "Olsen"), ("Kari", "Hansdatter"));
    let later = EventProfile {
        date: Some(on(1878, 10, 14)),
        ..church.clone()
    };
    let date = feature(&assess(&church, &later), Feature::Date).clone();
    assert!(matches!(date.outcome, Outcome::Partial(_)), "{date:?}");
}

#[test]
fn a_shared_origin_is_deterministic_and_two_items_of_one_record_conflict() {
    let (mut a, mut b) = (
        marriage(("Ole", "Olsen"), ("Kari", "Hansdatter")),
        marriage(("Ole", "Olsen"), ("Kari", "Hansdatter")),
    );
    a.origins.push(household(Some("event:1")));
    b.origins.push(household(Some("event:2")));
    let apart = assess(&a, &b);
    assert_eq!(feature(&apart, Feature::Record).outcome, Outcome::Conflict);
    assert_eq!(apart.band, MatchBand::Unlikely);
    b.origins = vec![household(Some("event:1"))];
    assert_eq!(assess(&a, &b).band, MatchBand::Deterministic);
}

fn event() -> impl Strategy<Value = EventProfile> {
    let principal = (given_name(), sex());
    (
        prop::bool::ANY,
        prop::option::of(a_date()),
        prop::collection::vec(principal, 0..3),
        prop::option::of(given_name()),
    )
        .prop_map(|(married, date, principals, witness)| {
            let mut event = marriage(("", ""), ("", ""));
            event.event_type = Some(if married {
                EventType::Marriage
            } else {
                EventType::Census
            });
            event.date = date;
            event.participants = principals
                .into_iter()
                .map(|(given, sex)| taking_part(ParticipantRole::Primary, given, "Olsen", sex))
                .collect();
            event
                .participants
                .extend(witness.map(|given| taking_part(ParticipantRole::Witness, given, "", Sex::Male)));
            event
        })
}

proptest! {
    #[test]
    fn an_event_score_is_a_probability(a in event(), b in event()) {
        let score = assess(&a, &b).score;
        prop_assert!((0.0..=1.0).contains(&score), "{score}");
    }

    #[test]
    fn an_event_matches_itself_at_least_as_well_as_any_other(a in event(), b in event()) {
        let (itself, other) = (assess(&a, &a), assess(&a, &b));
        prop_assert!(itself.score >= other.score, "self {} < other {}", itself.score, other.score);
    }

    #[test]
    fn an_event_assessment_is_symmetric(a in event(), b in event()) {
        let (ab, ba) = (assess(&a, &b), assess(&b, &a));
        prop_assert!((ab.score - ba.score).abs() < 1e-9, "{} vs {}", ab.score, ba.score);
        prop_assert_eq!(ab.band, ba.band);
    }
}
