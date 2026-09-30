//! The blocking keys' cases, and the property that blocking loses no pair the engine would show (ADR
//! 0038 §7).

use proptest::prelude::{Just, Strategy, prop, prop_assert, prop_oneof, proptest};

use super::{BlockingKeys, MatchableKind, Probe};
use crate::enums::{EventType, ParticipantRole, Sex};
use crate::ids::SourceId;
use crate::matching::pack::PackSource;
use crate::matching::profile::{
    CitationProfile, EventProfile, MediaProfile, NoteProfile, Participant, PersonProfile, PlaceProfile, SourceProfile,
    TagProfile, VitalKind,
};
use crate::matching::tests::{DATA, born, household, name, person, point, relative, vital};
use crate::matching::{DateBasis, MatchBand, MatchData, MatchSettings, assess_persons};
use crate::place_name::PlaceName;

fn keys() -> BlockingKeys<'static> {
    BlockingKeys::new(&DATA)
}

/// Whether a record keyed `a` probes a record keyed `b`, and the reverse — blocking is symmetric.
fn meet(a: &[String], b: &[String]) -> bool {
    let forward = b.iter().any(|key| Probe::of(a).meets(key));
    let backward = a.iter().any(|key| Probe::of(b).meets(key));
    assert_eq!(forward, backward, "blocking must be symmetric: {a:?} / {b:?}");
    forward
}

fn norwegian(given: &str, surname: &str, year: i32) -> PersonProfile {
    person(given, surname, Sex::Male, born(point(year, None, None), Some("Norway")))
}

fn persons_meet(a: &PersonProfile, b: &PersonProfile) -> bool {
    let keys = keys();
    meet(&keys.person(a), &keys.person(b))
}

#[test]
fn a_spelling_variant_meets() {
    assert!(persons_meet(
        &norwegian("Gulbrand", "Olsen", 1852),
        &norwegian("Guldbrand", "Olsøn", 1852)
    ));
}

#[test]
fn a_one_letter_slip_the_phonetic_key_keeps_meets() {
    assert!(persons_meet(
        &norwegian("Katherine", "Brown", 1852),
        &norwegian("Catherine", "Brown", 1852)
    ));
}

#[test]
fn names_of_one_equivalence_class_meet() {
    assert!(persons_meet(
        &norwegian("Jon", "Olsen", 1852),
        &norwegian("Johannes", "Olsen", 1852)
    ));
}

#[test]
fn a_census_age_five_years_off_meets() {
    let census = person(
        "Ole",
        "Olsen",
        Sex::Male,
        vec![vital(
            VitalKind::Birth,
            point(1852, None, None),
            DateBasis::FromAge,
            Some("Norway"),
        )],
    );
    assert!(persons_meet(&census, &norwegian("Ole", "Olsen", 1847)));
    assert!(persons_meet(
        &norwegian("Ole", "Olsen", 1849),
        &norwegian("Ole", "Olsen", 1859)
    ));
}

#[test]
fn a_baptism_standing_in_for_a_birth_meets() {
    let baptised = person(
        "Ole",
        "Olsen",
        Sex::Male,
        vec![vital(
            VitalKind::Baptism,
            point(1850, None, None),
            DateBasis::Recorded,
            None,
        )],
    );
    assert!(persons_meet(&baptised, &norwegian("Ole", "Olsen", 1849)));
}

#[test]
fn a_surname_changed_after_a_move_still_meets() {
    assert!(persons_meet(
        &norwegian("Ole", "Haugen", 1852),
        &norwegian("Ole", "Nordby", 1852)
    ));
}

#[test]
fn an_undated_person_meets_a_dated_one_of_any_decade() {
    let undated = person("Ole", "Olsen", Sex::Male, Vec::new());
    assert!(persons_meet(&undated, &norwegian("Ole", "Olsen", 1700)));
    assert!(persons_meet(&undated, &norwegian("Ole", "Olsen", 1900)));
}

#[test]
fn births_decades_apart_do_not_meet() {
    assert!(!persons_meet(
        &norwegian("Ole", "Olsen", 1850),
        &norwegian("Ole", "Olsen", 1900)
    ));
}

#[test]
fn different_names_do_not_meet() {
    assert!(!persons_meet(
        &norwegian("Ole", "Olsen", 1850),
        &norwegian("Kari", "Hansen", 1850)
    ));
}

#[test]
fn a_given_name_recorded_differently_meets_on_the_surname() {
    assert!(persons_meet(
        &norwegian("Katherine", "Olsen", 1874),
        &norwegian("Kari", "Olsen", 1880)
    ));
}

#[test]
fn a_person_with_only_a_surname_is_keyed_by_it() {
    let a = person("", "Olsen", Sex::Male, Vec::new());
    assert!(!keys().person(&a).is_empty());
    assert!(persons_meet(&a, &person("", "Olsen", Sex::Male, Vec::new())));
}

#[test]
fn a_shared_record_origin_meets_whatever_the_names() {
    let (mut a, mut b) = (norwegian("Ole", "Olsen", 1850), norwegian("Kari", "Hansen", 1900));
    a.origins.push(household(Some("person:1")));
    b.origins.push(household(Some("person:1")));
    assert!(persons_meet(&a, &b));
}

#[test]
fn a_family_is_keyed_by_its_partners() {
    let keys = keys();
    let family = |husband: &str, surname: &str| crate::matching::profile::FamilyProfile {
        partners: vec![norwegian(husband, surname, 1850)],
        ..Default::default()
    };
    assert!(meet(
        &keys.family(&family("Ole", "Olsen")),
        &keys.family(&family("Olle", "Olsen"))
    ));
    assert!(!meet(
        &keys.family(&family("Ole", "Olsen")),
        &keys.family(&family("Kari", "Hansen"))
    ));
}

#[test]
fn an_event_is_keyed_by_its_principals_by_decade() {
    let keys = keys();
    let baptism = |given: &str, year: i32| EventProfile {
        event_type: Some(EventType::Baptism),
        date: Some(point(year, None, None)),
        participants: vec![
            Participant {
                role: ParticipantRole::Primary,
                person: relative(given, "Olsen", Sex::Male, None),
            },
            Participant {
                role: ParticipantRole::Witness,
                person: relative("Kari", "Hansen", Sex::Female, None),
            },
        ],
        ..EventProfile::default()
    };
    assert!(meet(
        &keys.event(&baptism("Ole", 1850)),
        &keys.event(&baptism("Ola", 1851))
    ));
    assert!(!meet(
        &keys.event(&baptism("Ole", 1850)),
        &keys.event(&baptism("Per", 1850))
    ));
    assert!(!meet(
        &keys.event(&baptism("Ole", 1850)),
        &keys.event(&baptism("Ole", 1900))
    ));
}

fn place(text: &str) -> PlaceProfile {
    PlaceProfile {
        names: vec![PlaceName {
            text: text.to_owned(),
            language: None,
            date: None,
        }],
        ..PlaceProfile::default()
    }
}

#[test]
fn a_place_is_keyed_by_its_names() {
    let keys = keys();
    assert!(meet(&keys.place(&place("Nordaas")), &keys.place(&place("Nordås"))));
    assert!(!meet(&keys.place(&place("Nordås")), &keys.place(&place("Bergen"))));
}

fn source(title: &str) -> SourceProfile {
    SourceProfile {
        title: Some(title.to_owned()),
        ..SourceProfile::default()
    }
}

#[test]
fn a_source_is_keyed_by_its_title_words() {
    let keys = keys();
    assert!(meet(
        &keys.source(&source("Ministerialbok for Fana")),
        &keys.source(&source("Fana ministerialbok"))
    ));
    assert!(!meet(
        &keys.source(&source("Fana ministerialbok")),
        &keys.source(&source("Folketelling 1865"))
    ));
}

#[test]
fn citations_of_one_source_meet_on_a_page_word_or_an_unknown_page() {
    let keys = keys();
    let id = SourceId::from_uuid(uuid::Uuid::now_v7());
    let citation = |page: Option<&str>| CitationProfile {
        source: Some(SourceProfile {
            id: Some(id),
            ..SourceProfile::default()
        }),
        page: page.map(ToOwned::to_owned),
        ..CitationProfile::default()
    };
    assert!(meet(
        &keys.citation(&citation(Some("s. 12"))),
        &keys.citation(&citation(Some("side 12")))
    ));
    assert!(meet(
        &keys.citation(&citation(Some("s. 12"))),
        &keys.citation(&citation(None))
    ));
    assert!(!meet(
        &keys.citation(&citation(Some("12"))),
        &keys.citation(&citation(Some("40")))
    ));
}

#[test]
fn media_meet_on_a_checksum() {
    let keys = keys();
    let media = |checksum: &str| MediaProfile {
        checksum: Some(checksum.to_owned()),
        ..MediaProfile::default()
    };
    assert!(meet(&keys.media(&media("ABC123")), &keys.media(&media("abc123"))));
    assert!(!meet(&keys.media(&media("abc123")), &keys.media(&media("def456"))));
}

#[test]
fn notes_meet_on_their_long_words() {
    let keys = keys();
    let note = |text: &str| NoteProfile {
        text: Some(text.to_owned()),
        ..NoteProfile::default()
    };
    assert!(meet(
        &keys.note(&note("Utvandret til Amerika i 1882")),
        &keys.note(&note("Utvandret til Amerika 1882"))
    ));
    assert!(!meet(
        &keys.note(&note("Utvandret til Amerika")),
        &keys.note(&note("Konfirmert i Fana"))
    ));
}

#[test]
fn tags_meet_on_the_folded_name() {
    let keys = keys();
    let tag = |name: &str| TagProfile { name: name.to_owned() };
    assert!(meet(&keys.tag(&tag("Emigrant ")), &keys.tag(&tag("emigrant"))));
    assert!(!meet(&keys.tag(&tag("Emigrant")), &keys.tag(&tag("Soldier"))));
}

#[test]
fn every_kind_names_its_aggregate_type() {
    for kind in MatchableKind::ALL {
        assert_eq!(MatchableKind::parse(kind.as_str()), Some(kind));
    }
    assert_eq!(MatchableKind::parse("dna_test"), None);
}

#[test]
fn the_fingerprint_is_stable_and_follows_the_packs() {
    let toy = include_str!("../../../tests/fixtures/matching/toy.toml");
    let with_toy = MatchData::embedded()
        .unwrap()
        .layered([PackSource::new("toy.toml", toy)], None)
        .unwrap();
    assert_eq!(
        keys().fingerprint(),
        BlockingKeys::new(&MatchData::embedded().unwrap()).fingerprint()
    );
    assert_ne!(keys().fingerprint(), BlockingKeys::new(&with_toy).fingerprint());
}

#[test]
fn a_toy_pack_class_becomes_a_key() {
    let toy = include_str!("../../../tests/fixtures/matching/toy.toml");
    let data = MatchData::embedded()
        .unwrap()
        .layered([PackSource::new("toy.toml", toy)], None)
        .unwrap();
    let keys = BlockingKeys::new(&data);
    let speaker = |given: &str| person(given, "", Sex::Male, Vec::new());
    assert!(meet(&keys.person(&speaker("Zorbo")), &keys.person(&speaker("Quimble"))));
    assert!(!persons_meet(&speaker("Zorbo"), &speaker("Quimble")));
}

#[test]
fn a_probe_reaches_neighbouring_decades_and_the_unknown_one() {
    let probe = Probe::of(&["t:ole@185".to_owned(), "t:kari@?".to_owned(), "sum:ab".to_owned()]);
    for key in ["t:ole@184", "t:ole@185", "t:ole@186", "t:ole@?", "t:kari@170", "sum:ab"] {
        assert!(probe.meets(key), "{key}");
    }
    for key in ["t:ole@187", "t:olea@185", "sum:abc"] {
        assert!(!probe.meets(key), "{key}");
    }
}

fn given_names() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("Ole"),
        Just("Olle"),
        Just("Ola"),
        Just("Olav"),
        Just("Gulbrand"),
        Just("Guldbrand"),
        Just("Jon"),
        Just("Johannes"),
        Just("Johan"),
        Just("Hans"),
        Just("Kari"),
        Just("Karen"),
        Just("Katherine"),
        Just("Catherine"),
        Just("Kristian"),
        Just("Christian"),
        Just("Anne"),
        Just("Anna"),
    ]
}

fn surnames() -> impl Strategy<Value = &'static str> {
    prop_oneof![
        Just("Olsen"),
        Just("Olsøn"),
        Just("Hansen"),
        Just("Haugen"),
        Just("Haug"),
        Just("Nordby"),
        Just(""),
    ]
}

fn profiles() -> impl Strategy<Value = PersonProfile> {
    let kind = prop_oneof![Just(VitalKind::Birth), Just(VitalKind::Baptism)];
    let basis = prop_oneof![Just(DateBasis::Recorded), Just(DateBasis::FromAge)];
    let country = prop_oneof![Just(Some("Norway")), Just(None)];
    let vitals = prop::option::of((kind, 1800i32..1900, basis, country))
        .prop_map(|v| v.map(|(kind, year, basis, country)| vital(kind, point(year, None, None), basis, country)));
    (given_names(), surnames(), vitals).prop_map(|(given, surname, vital)| PersonProfile {
        names: vec![name(given, surname)],
        sex: Some(Sex::Male),
        vitals: vital.into_iter().collect(),
        ..PersonProfile::default()
    })
}

proptest! {
    /// Every pair the engine would show is a candidate: blocking never loses recall.
    #[test]
    fn every_pair_the_engine_shows_meets(a in profiles(), b in profiles()) {
        let assessment = assess_persons(&a, &b, &DATA, &MatchSettings::default());
        if assessment.band >= MatchBand::Possible {
            prop_assert!(persons_meet(&a, &b), "{a:?}\n{b:?}\n{:?}", assessment.score);
        }
    }
}
