//! The harness's parsing, tallies and gate, and the shipped corpus against the real engine.

use std::path::Path;

use vitni_core::date::{DateModifier, GenealogicalDateBody};
use vitni_core::enums::Sex;
use vitni_core::matching::profile::VitalKind;
use vitni_core::matching::{DateBasis, MatchBand, MatchData, MatchSettings};

use crate::match_eval::corpus::{HardCase, Label, Pair, parse};
use crate::match_eval::{CORPUS_DIR, Report, Scored, gate, load, score};

const CENSUS_AGE: &str = r#"
[[pair]]
id = "census-age"
label = "same"
hard = "census-age"
source = "invented"
[pair.left]
given = "Ole"
surname = "Olsen"
sex = "male"
occupations = ["husmann"]
birth = { year = 1852, basis = "age", country = "Norge", place = "Vik" }
parents = [{ given = "Ole", surname = "Hansen", sex = "male", born = 1815 }]
[pair.right]
given = "Ole"
surname = "Olsen"
sex = "male"
baptism = { year = 1849, month = 6, day = 10 }
"#;

fn one(text: &str) -> Pair {
    let mut pairs = parse("test.toml", text).unwrap();
    assert_eq!(pairs.len(), 1);
    pairs.remove(0)
}

fn year(vital: &vitni_core::matching::profile::VitalEvent) -> Option<i32> {
    let GenealogicalDateBody::Structured(DateModifier::None(point)) = &vital.date.as_ref()?.modifier else {
        return None;
    };
    point.year
}

#[test]
fn a_record_maps_onto_a_person_profile() {
    let pair = one(CENSUS_AGE);
    assert_eq!(pair.id, "census-age");
    assert_eq!(pair.label, Label::Same);
    assert_eq!(pair.hard, Some(HardCase::CensusAge));

    let left = &pair.left;
    assert_eq!(left.sex, Some(Sex::Male));
    assert_eq!(left.names[0].given.as_deref(), Some("Ole"));
    assert_eq!(left.names[0].surnames[0].surname, "Olsen");
    assert_eq!(left.occupations, ["husmann"]);
    let birth = &left.vitals[0];
    assert_eq!(birth.kind, VitalKind::Birth);
    assert_eq!(birth.basis, DateBasis::FromAge);
    assert_eq!(year(birth), Some(1852));
    let place = birth.place.as_ref().unwrap();
    assert_eq!(place.country.as_deref(), Some("Norge"));
    assert_eq!(place.names[0].text, "Vik");
    let father = &left.parents[0];
    assert_eq!(father.names[0].given.as_deref(), Some("Ole"));
    assert_eq!(father.birth.as_ref().and_then(year), Some(1815));

    let baptism = &pair.right.vitals[0];
    assert_eq!(baptism.kind, VitalKind::Baptism);
    assert_eq!(baptism.basis, DateBasis::Recorded);
    assert!(baptism.place.is_none());
}

#[test]
fn an_unknown_field_is_rejected_naming_the_file() {
    let text = CENSUS_AGE.replace("occupations", "ocupations");
    let error = format!("{:#}", parse("census.toml", &text).unwrap_err());
    assert!(error.contains("census.toml"), "{error}");
    assert!(error.contains("ocupations"), "{error}");
}

#[test]
fn an_unknown_hard_case_is_rejected() {
    let text = CENSUS_AGE.replace("hard = \"census-age\"", "hard = \"typo\"");
    assert!(parse("test.toml", &text).is_err());
}

#[test]
fn a_hard_distinct_pair_is_rejected_naming_it() {
    let text = CENSUS_AGE.replace("label = \"same\"", "label = \"distinct\"");
    let error = format!("{:#}", parse("test.toml", &text).unwrap_err());
    assert!(error.contains("census-age"), "{error}");
}

#[test]
fn a_date_the_engine_would_drop_or_widen_is_rejected() {
    for (bad, needle) in [
        ("month = 13, day = 10", "month"),
        ("month = 6, day = 0", "day"),
        ("day = 10", "needs a month"),
    ] {
        let text = CENSUS_AGE.replace("month = 6, day = 10", bad);
        let error = format!("{:#}", parse("test.toml", &text).unwrap_err());
        assert!(error.contains("census-age") && error.contains(needle), "{bad}: {error}");
    }
}

#[test]
fn an_empty_record_is_rejected_naming_the_pair() {
    let text = "[[pair]]\nid = \"empty\"\nlabel = \"same\"\nsource = \"invented\"\n[pair.left]\n[pair.right]\n";
    let error = format!("{:#}", parse("test.toml", text).unwrap_err());
    assert!(error.contains("empty"), "{error}");
}

#[test]
fn a_duplicate_id_across_files_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("a.toml"), CENSUS_AGE).unwrap();
    std::fs::write(dir.path().join("b.toml"), CENSUS_AGE).unwrap();
    let error = format!("{:#}", load(dir.path()).unwrap_err());
    assert!(error.contains("census-age"), "{error}");
}

#[test]
fn an_empty_corpus_directory_is_rejected() {
    let dir = tempfile::tempdir().unwrap();
    assert!(load(dir.path()).is_err());
}

fn scored(id: &str, label: Label, hard: Option<HardCase>, band: MatchBand) -> Scored {
    Scored {
        id: id.to_owned(),
        label,
        hard,
        band,
        score: 0.5,
        blocked: false,
    }
}

fn every_hard_case_surfacing() -> Vec<Scored> {
    let mut results = Vec::new();
    for hard in HardCase::ALL {
        results.push(scored(hard.as_str(), Label::Same, Some(hard), MatchBand::Possible));
    }
    results
}

#[test]
fn the_report_counts_each_band_and_its_precision() {
    let results = vec![
        scored("a", Label::Same, None, MatchBand::Probable),
        scored("b", Label::Same, None, MatchBand::Probable),
        scored("c", Label::Distinct, None, MatchBand::Probable),
        scored("d", Label::Same, None, MatchBand::Possible),
        scored("e", Label::Distinct, None, MatchBand::Unlikely),
        Scored {
            blocked: true,
            ..scored("f", Label::Same, None, MatchBand::Probable)
        },
    ];
    let report = Report::of(&results);
    let probable = report.row(MatchBand::Probable);
    assert_eq!((probable.same, probable.distinct), (2, 1));
    assert_eq!(probable.precision(), Some(2.0 / 3.0));
    assert_eq!(report.recall_at(MatchBand::Probable), Some(2.0 / 4.0));
    assert_eq!(report.recall_at(MatchBand::Possible), Some(3.0 / 4.0));
    assert_eq!(report.blocked_same, 1);
    assert_eq!(report.row(MatchBand::Deterministic).precision(), None);
}

#[test]
fn an_empty_report_has_no_rates() {
    let report = Report::of(&[]);
    assert_eq!(report.recall_at(MatchBand::Possible), None);
    assert!(!report.render().contains("NaN"));
}

#[test]
fn the_gate_passes_when_every_hard_case_surfaces() {
    let mut results = every_hard_case_surfacing();
    results.push(scored("stranger", Label::Distinct, None, MatchBand::Probable));
    gate(&results).unwrap();
}

#[test]
fn the_gate_fails_naming_a_hard_case_that_falls_below_possible() {
    let mut results = every_hard_case_surfacing();
    results.push(scored(
        "lost-spelling",
        Label::Same,
        Some(HardCase::SpellingVariant),
        MatchBand::Unlikely,
    ));
    let error = format!("{:#}", gate(&results).unwrap_err());
    assert!(error.contains("lost-spelling"), "{error}");
}

#[test]
fn the_gate_fails_naming_a_hard_case_blocking_loses() {
    let mut results = every_hard_case_surfacing();
    results.push(Scored {
        blocked: true,
        ..scored("unindexed", Label::Same, Some(HardCase::CensusAge), MatchBand::Probable)
    });
    let error = format!("{:#}", gate(&results).unwrap_err());
    assert!(error.contains("unindexed"), "{error}");
    assert!(error.contains("blocking"), "{error}");
}

#[test]
fn the_gate_fails_when_a_hard_case_has_no_pair() {
    let mut results = every_hard_case_surfacing();
    results.retain(|result| result.hard != Some(HardCase::BaptismForBirth));
    let error = format!("{:#}", gate(&results).unwrap_err());
    assert!(error.contains(HardCase::BaptismForBirth.as_str()), "{error}");
}

#[test]
fn the_shipped_corpus_passes_the_gate() {
    let pairs = load(&Path::new(env!("CARGO_MANIFEST_DIR")).join("..").join(CORPUS_DIR)).unwrap();
    let data = MatchData::embedded().unwrap();
    let settings = MatchSettings::default();
    let mut results = Vec::new();
    for pair in &pairs {
        results.push(score(pair, &data, &settings));
    }
    assert!(results.iter().any(|result| result.label == Label::Distinct));
    gate(&results).unwrap();
}

#[test]
fn the_report_names_every_pair_on_the_wrong_side_of_possible() {
    let results = vec![
        scored("found", Label::Same, None, MatchBand::Probable),
        scored("missed", Label::Same, None, MatchBand::Unlikely),
        scored("rejected", Label::Distinct, None, MatchBand::Unlikely),
        scored("confused", Label::Distinct, None, MatchBand::Possible),
    ];
    let rendered = Report::of(&results).render();
    assert!(rendered.contains("missed [same]: unlikely"), "{rendered}");
    assert!(rendered.contains("confused [distinct]: possible"), "{rendered}");
    assert!(!rendered.contains("found"), "{rendered}");
    assert!(!rendered.contains("rejected"), "{rendered}");
}
