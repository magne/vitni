use super::{DuplicateCandidateVm, MergeCompareVm, MergeFailure, MergeResultVm};
use crate::i18n::Localizer;
use std::collections::BTreeSet;
use vitni_app::{AggRef, Confidence, FactSummary, MatchAssessment, MatchBand, MergeResult, PersonSummary, SimilarPair};
use vitni_app::{Fact, FactType};

fn agg(human_id: &str) -> AggRef {
    AggRef {
        human_id: human_id.to_owned(),
        id: format!("{human_id}-id"),
    }
}

fn bare_summary(human_id: &str, display_name: Option<&str>) -> PersonSummary {
    PersonSummary {
        human_id: human_id.to_owned(),
        evidence_level: vitni_app::EvidenceLevel::Conclusion,
        display_name: display_name.map(ToOwned::to_owned),
        given: None,
        surname: None,
        surname_prefix: None,
        nickname: None,
        name_prefix: None,
        name_suffix: None,
        name_type: None,
        primary_name_assertion: None,
        names: Vec::new(),
        sex: None,
        birth_date: None,
        death_date: None,
        facts: Vec::new(),
        associations: Vec::new(),
        participations: Vec::new(),
        citations: Vec::new(),
        media: Vec::new(),
        notes: Vec::new(),
        tags: Vec::new(),
        tag_refs: Vec::new(),
        restrictions: BTreeSet::new(),
        merged: Vec::new(),
        claim_owners: std::collections::BTreeMap::new(),
    }
}

#[test]
fn a_duplicate_pair_shows_the_engine_score_as_a_percentage_and_its_band() {
    let loc = Localizer::for_test("en");
    let pair = SimilarPair {
        a: agg("I0042"),
        b: agg("I0099"),
        assessment: MatchAssessment {
            score: 0.936,
            band: MatchBand::Possible,
            features: Vec::new(),
            cultures: Vec::new(),
            parts: Vec::new(),
            engine: vitni_app::ENGINE_VERSION,
        },
    };
    let vm = DuplicateCandidateVm::build(&pair, &loc);
    assert_eq!(vm.a.human_id, "I0042");
    assert_eq!(vm.b.human_id, "I0099");
    assert_eq!(vm.score, 94, "the engine score as a percentage, not a confidence level");
    assert_eq!(vm.reason, "possible match");
    assert!(vm.reasons.is_empty(), "no terms, nothing to explain");
}

#[test]
fn a_duplicate_pair_explains_its_score_by_the_engines_terms() {
    use vitni_app::{Feature, FeatureComparison, Outcome};
    let loc = Localizer::for_test("en");
    let term = |feature, outcome, weight| FeatureComparison {
        feature,
        outcome,
        weight,
        left: None,
        right: None,
    };
    let mut shown = assessment(0.97, MatchBand::Probable);
    shown.features = vec![
        term(Feature::Surname, Outcome::Partial(0.9), 1.5),
        term(Feature::Birth, Outcome::Missing, 0.0),
        term(Feature::GivenName, Outcome::Agree, 3.25),
    ];
    let pair = SimilarPair {
        a: agg("I0042"),
        b: agg("I0099"),
        assessment: shown,
    };
    let vm = DuplicateCandidateVm::build(&pair, &loc);
    assert_eq!(vm.reasons, ["Same given name (+3.3)", "Similar surname (+1.5)"]);
}

#[test]
fn compare_grid_carries_only_real_fields() {
    let loc = Localizer::for_test("en");
    let mut survivor = bare_summary("I0042", Some("John Smith"));
    survivor.facts.push(FactSummary {
        fact: Fact {
            fact_type: FactType::Occupation,
            date: None,
            place_id: None,
            value: Some("Carpenter".to_owned()),
        },
        confidence: Some(Confidence::Normal),
        citations: Vec::new(),
        assertion_id: "aaaaaaaa-0000-7000-8000-00000000000d".to_owned(),
    });
    let merged = bare_summary("I0099", Some("John Smyth"));

    let vm = MergeCompareVm::build(&survivor, &merged, &assessment(0.5, MatchBand::Possible), &loc);
    assert_eq!(vm.survivor.human_id, "I0042");
    assert_eq!(vm.merged.human_id, "I0099");
    let occupation = vm
        .fields
        .iter()
        .find(|row| row.survivor_value.as_deref() == Some("Carpenter"))
        .expect("occupation row present");
    assert_eq!(occupation.merged_value, None, "merged has no occupation recorded");
}

#[test]
fn merge_result_summary_never_claims_repointing() {
    let loc = Localizer::for_test("en");
    let result = MergeResult {
        survivor: bare_summary("I0042", Some("John Smith")),
        merged_human_id: "I0099".to_owned(),
    };
    let vm = MergeResultVm::build(&result, &loc);
    assert!(
        !vm.summary.to_lowercase().contains("re-point") && !vm.summary.to_lowercase().contains("repoint"),
        "must not claim re-pointing: {}",
        vm.summary
    );
    assert!(vm.summary.contains("I0099"));
    assert!(vm.summary.contains("I0042"));
}

fn assessment(score: f64, band: MatchBand) -> MatchAssessment {
    MatchAssessment {
        score,
        band,
        features: Vec::new(),
        cultures: Vec::new(),
        parts: Vec::new(),
        engine: vitni_app::EngineVersion(4),
    }
}

#[test]
fn the_compare_view_carries_the_assessment_the_decision_will_record() {
    let loc = Localizer::for_test("en");
    let shown = assessment(0.9712, MatchBand::Probable);
    let vm = MergeCompareVm::build(
        &bare_summary("I0042", Some("John Smith")),
        &bare_summary("I0099", Some("John Smyth")),
        &shown,
        &loc,
    );
    assert_eq!(vm.assessment, shown.evidence());
    assert_eq!(vm.assessment_line, "Matched at 97% · probable match · engine 4");
}

#[test]
fn a_pair_already_decided_is_a_blocked_decision_not_a_toast() {
    let loc = Localizer::for_test("en");
    let error = vitni_app::AppError::Domain(vitni_app::PersonError::IdentityDecided {
        person: vitni_app::PersonId::from_uuid(uuid::Uuid::from_u128(1)),
        other: vitni_app::PersonId::from_uuid(uuid::Uuid::from_u128(2)),
    });
    let MergeFailure::Blocked(vm) = MergeFailure::from_error(&error, &loc) else {
        panic!("an already-decided pair renders the blocked card");
    };
    assert_eq!(vm.heading, "Already decided");
    assert!(vm.guidance.contains("Undo it in History"), "{}", vm.guidance);
}
