use super::{MatchQueueVm, MergeFailure};
use crate::i18n::Localizer;
use crate::navigation::Category;
use vitni_app::{AggRef, DecidableKind, MatchAssessment, MatchBand, QueuedMatch};

fn agg(human_id: &str) -> AggRef {
    AggRef {
        human_id: human_id.to_owned(),
        id: format!("{human_id}-id"),
    }
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

fn queued(kind: DecidableKind, a: &str, b: &str, score: f64) -> QueuedMatch {
    QueuedMatch {
        kind,
        a: agg(a),
        b: agg(b),
        assessment: assessment(score, MatchBand::Possible),
    }
}

#[test]
fn a_queued_pair_shows_its_kind_the_engine_score_as_a_percentage_and_its_band() {
    let loc = Localizer::for_test("en");
    let vm = MatchQueueVm::build(&[queued(DecidableKind::Person, "I0042", "I0099", 0.936)], &[], &loc);
    let pair = &vm.pairs[0];
    assert_eq!(pair.kind, DecidableKind::Person);
    assert_eq!(pair.kind_label, "Person");
    assert_eq!((pair.a.human_id.as_str(), pair.b.human_id.as_str()), ("I0042", "I0099"));
    assert_eq!(
        pair.percent, 94,
        "the engine score as a percentage, not a confidence level"
    );
    assert_eq!(pair.band, "possible match");
    assert!(pair.reasons.is_empty(), "no terms, nothing to explain");
    assert_eq!(vm.runs, []);
}

#[test]
fn a_queued_pair_of_any_kind_links_to_its_own_category() {
    let loc = Localizer::for_test("en");
    let queue = [
        queued(DecidableKind::Place, "P0001", "P0002", 0.8),
        queued(DecidableKind::Source, "S0001", "S0002", 0.7),
        queued(DecidableKind::Media, "O0001", "O0002", 0.6),
    ];
    let vm = MatchQueueVm::build(&queue, &[], &loc);
    let categories: Vec<Category> = vm.pairs.iter().map(|pair| pair.b.category).collect();
    assert_eq!(categories, [Category::Places, Category::Sources, Category::Media]);
    let labels: Vec<&str> = vm.pairs.iter().map(|pair| pair.kind_label.as_str()).collect();
    assert_eq!(labels, ["Place", "Source", "Media object"]);
}

#[test]
fn every_kind_has_a_label_in_each_language() {
    for language in ["en", "no"] {
        let loc = Localizer::for_test(language);
        for kind in DecidableKind::ALL {
            let label = loc.match_kind(kind);
            assert!(
                !label.is_empty() && !label.contains('{'),
                "{language} {kind:?}: {label}"
            );
        }
    }
}

#[test]
fn a_decision_summary_names_both_records_and_never_claims_repointing() {
    let loc = Localizer::for_test("en");
    let person = loc.match_merged_summary(DecidableKind::Person, "I0099", "I0042");
    assert_eq!(person, "I0099 becomes a persona of I0042; one event added to History.");
    let place = loc.match_merged_summary(DecidableKind::Place, "P0002", "P0001");
    assert_eq!(place, "P0002 is merged into P0001; one event added to History.");
    let distinct = loc.match_distinguished_summary(DecidableKind::Place, "P0002", "P0001");
    assert_eq!(
        distinct,
        "P0002 is marked as a different place from P0001; one event added to History."
    );
    for summary in [person, place, distinct] {
        assert!(!summary.to_lowercase().contains("point"), "{summary}");
    }
}

#[test]
fn a_place_pair_already_decided_is_a_blocked_decision_not_a_toast() {
    let loc = Localizer::for_test("en");
    let error = vitni_app::AppError::PlaceDomain(vitni_app::PlaceError::IdentityDecided {
        place: vitni_app::PlaceId::from_uuid(uuid::Uuid::from_u128(1)),
        other: vitni_app::PlaceId::from_uuid(uuid::Uuid::from_u128(2)),
    });
    let MergeFailure::Blocked(vm) = MergeFailure::from_error(&error, &loc) else {
        panic!("an already-decided pair renders the blocked card");
    };
    assert_eq!(vm.heading, "Already decided");
    assert!(vm.guidance.contains("Undo it in History"), "{}", vm.guidance);
}

#[test]
fn a_person_pair_already_decided_is_a_blocked_decision_not_a_toast() {
    let loc = Localizer::for_test("en");
    let error = vitni_app::AppError::Domain(vitni_app::PersonError::IdentityDecided {
        person: vitni_app::PersonId::from_uuid(uuid::Uuid::from_u128(1)),
        other: vitni_app::PersonId::from_uuid(uuid::Uuid::from_u128(2)),
    });
    assert!(matches!(
        MergeFailure::from_error(&error, &loc),
        MergeFailure::Blocked(_)
    ));
}

#[test]
fn any_other_error_is_a_toast() {
    let loc = Localizer::for_test("en");
    let error = vitni_app::AppError::PlaceNotFound("P0009".to_owned());
    assert!(matches!(MergeFailure::from_error(&error, &loc), MergeFailure::Other(_)));
}
