use super::{ConfidenceLevel, DuplicateCandidateVm, MergeCompareVm, MergeResultVm};
use crate::i18n::Localizer;
use genealogy_app::{AggRef, Confidence, DuplicateCandidate, FactSummary, MatchKind, MergeResult, PersonSummary};
use genealogy_app::{Fact, FactType};
use std::collections::BTreeSet;

fn agg(human_id: &str) -> AggRef {
    AggRef {
        human_id: human_id.to_owned(),
        id: format!("{human_id}-id"),
    }
}

fn bare_summary(human_id: &str, display_name: Option<&str>) -> PersonSummary {
    PersonSummary {
        human_id: human_id.to_owned(),
        evidence_level: genealogy_app::EvidenceLevel::Conclusion,
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
    }
}

#[test]
fn duplicate_candidate_maps_score_and_localizes_reason() {
    let loc = Localizer::for_test("en");
    let candidate = DuplicateCandidate {
        a: agg("I0042"),
        b: agg("I0099"),
        kind: MatchKind::NameVariant,
        score: 94,
    };
    let vm = DuplicateCandidateVm::build(&candidate, &loc);
    assert_eq!(vm.a.human_id, "I0042");
    assert_eq!(vm.b.human_id, "I0099");
    assert_eq!(vm.confidence, ConfidenceLevel::VeryHigh);
    assert!(!vm.reason.is_empty());
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
            citations: Vec::new(),
        },
        confidence: Confidence::Normal,
        citations: Vec::new(),
        assertion_id: "aaaaaaaa-0000-7000-8000-00000000000d".to_owned(),
    });
    let merged = bare_summary("I0099", Some("John Smyth"));

    let vm = MergeCompareVm::build(&survivor, &merged, &loc);
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
        still_referenced: 3,
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
