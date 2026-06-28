//! The pure `DnaMatch` decision core (ADR 0004 §3) and the `evolve` fold.
//!
//! The observed match is high-surety data; the *relationship it implies* is a separate, citing
//! assertion on Person/Family (data-model §12), not modelled here. Both tests must exist
//! (`refs`, resolved by the [`DnaMatchRefResolver`](super::ref_resolver)); same-test and
//! negative-cM are within-aggregate checks.

use crate::assertions::Attributed;
use crate::dna_match::command::DnaMatchCommand;
use crate::dna_match::error::DnaMatchError;
use crate::dna_match::event::{DnaMatchEvent, DnaMatchEventBody};
use crate::dna_match::ref_resolver::DnaMatchRefs;
use crate::dna_match::state::{DnaMatchState, MatchStatus};
use crate::ids::DnaMatchId;
use crate::provenance::AssertionMeta;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`DnaMatchError`] when the command violates an invariant: observing a match that
/// exists, referencing a missing test, the same test on both sides, a negative shared-cM total, a
/// command against an absent match, or correcting an unknown assertion.
pub fn decide(
    state: &DnaMatchState,
    command: DnaMatchCommand,
    meta: &AssertionMeta,
    refs: &DnaMatchRefs,
) -> Result<Vec<DnaMatchEvent>, DnaMatchError> {
    match command {
        DnaMatchCommand::ObserveMatch {
            dna_match_id,
            human_id,
            test_a,
            test_b,
            provider,
            shared_cm,
            percent_shared,
            segment_count,
            largest_segment_cm,
            predicted_relationship,
        } => {
            if state.exists {
                return Err(DnaMatchError::AlreadyExists(dna_match_id));
            }
            if test_a == test_b {
                return Err(DnaMatchError::SameTestBothSides(test_a));
            }
            if !refs.test_a_exists {
                return Err(DnaMatchError::UnknownTest(test_a));
            }
            if !refs.test_b_exists {
                return Err(DnaMatchError::UnknownTest(test_b));
            }
            if shared_cm.as_hundredths() < 0 {
                return Err(DnaMatchError::NegativeSharedCm);
            }
            Ok(one(
                meta,
                DnaMatchEventBody::DnaMatchObserved {
                    dna_match_id,
                    human_id,
                    test_a,
                    test_b,
                    provider,
                    shared_cm,
                    percent_shared,
                    segment_count,
                    largest_segment_cm,
                    predicted_relationship,
                },
            ))
        }
        // The plain commands share the same shape — exist-check then emit one event — so they
        // delegate to `simple_body` (exhaustive over them). Only `dna_match_id` is bound here (it
        // is `Copy`), leaving `command` intact to hand over.
        DnaMatchCommand::AddSegment { dna_match_id, .. }
        | DnaMatchCommand::AssertSharedAncestor { dna_match_id, .. }
        | DnaMatchCommand::ConfirmMatch { dna_match_id }
        | DnaMatchCommand::RejectMatch { dna_match_id }
        | DnaMatchCommand::AttachNote { dna_match_id, .. }
        | DnaMatchCommand::Tag { dna_match_id, .. }
        | DnaMatchCommand::Untag { dna_match_id, .. }
        | DnaMatchCommand::SetRestrictions { dna_match_id, .. } => {
            ensure_exists(state, dna_match_id)?;
            Ok(one(meta, simple_body(command)))
        }
        DnaMatchCommand::RetractAssertion { dna_match_id, target } => {
            ensure_exists(state, dna_match_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(DnaMatchError::RetractsMissingAssertion(target));
            }
            Ok(one(
                meta,
                DnaMatchEventBody::AssertionRetracted { dna_match_id, target },
            ))
        }
        DnaMatchCommand::SupersedeAssertion {
            dna_match_id,
            target,
            replacement,
        } => {
            ensure_exists(state, dna_match_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(DnaMatchError::SupersedesMissingAssertion(target));
            }
            let mut events = one(meta, DnaMatchEventBody::AssertionSuperseded { dna_match_id, target });
            events.extend(decide(state, *replacement, meta, refs)?);
            Ok(events)
        }
    }
}

/// Maps a plain command to its event body (the existence check is done by `decide`).
///
/// Exhaustive over the plain commands; the lifecycle/cross-aggregate commands never reach here.
fn simple_body(command: DnaMatchCommand) -> DnaMatchEventBody {
    match command {
        DnaMatchCommand::AddSegment { dna_match_id, segment } => {
            DnaMatchEventBody::SegmentAdded { dna_match_id, segment }
        }
        DnaMatchCommand::AssertSharedAncestor { dna_match_id, ancestor } => {
            DnaMatchEventBody::SharedAncestorAsserted { dna_match_id, ancestor }
        }
        DnaMatchCommand::ConfirmMatch { dna_match_id } => DnaMatchEventBody::MatchConfirmed { dna_match_id },
        DnaMatchCommand::RejectMatch { dna_match_id } => DnaMatchEventBody::MatchRejected { dna_match_id },
        DnaMatchCommand::AttachNote { dna_match_id, note_id } => {
            DnaMatchEventBody::NoteAttached { dna_match_id, note_id }
        }
        DnaMatchCommand::Tag { dna_match_id, tag_id } => DnaMatchEventBody::Tagged { dna_match_id, tag_id },
        DnaMatchCommand::Untag { dna_match_id, tag_id } => DnaMatchEventBody::Untagged { dna_match_id, tag_id },
        DnaMatchCommand::SetRestrictions {
            dna_match_id,
            restrictions,
        } => DnaMatchEventBody::RestrictionsChanged {
            dna_match_id,
            restrictions,
        },
        DnaMatchCommand::ObserveMatch { .. }
        | DnaMatchCommand::RetractAssertion { .. }
        | DnaMatchCommand::SupersedeAssertion { .. } => unreachable!("handled by decide"),
    }
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: DnaMatchEventBody) -> Vec<DnaMatchEvent> {
    vec![DnaMatchEvent::new(meta, body)]
}

/// Rejects a command that targets a match which has not been observed yet.
fn ensure_exists(state: &DnaMatchState, dna_match_id: DnaMatchId) -> Result<(), DnaMatchError> {
    if state.exists {
        Ok(())
    } else {
        Err(DnaMatchError::NotFound(dna_match_id))
    }
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut DnaMatchState, event: &DnaMatchEvent) {
    let assertion_id = event.assertion_id;
    match &event.body {
        DnaMatchEventBody::DnaMatchObserved {
            dna_match_id,
            human_id,
            test_a,
            test_b,
            provider,
            shared_cm,
            percent_shared,
            segment_count,
            largest_segment_cm,
            predicted_relationship,
        } => {
            state.exists = true;
            state.dna_match_id = Some(*dna_match_id);
            state.human_id = Some(human_id.clone());
            state.test_a = Some(*test_a);
            state.test_b = Some(*test_b);
            state.provider = Some(provider.clone());
            state.shared_cm = Some(*shared_cm);
            state.percent_shared = *percent_shared;
            state.segment_count = Some(*segment_count);
            state.largest_segment_cm = Some(*largest_segment_cm);
            state.predicted_relationship.clone_from(predicted_relationship);
            state.live_assertions.insert(assertion_id);
        }
        DnaMatchEventBody::SegmentAdded { segment, .. } => {
            state.segments.push(Attributed {
                assertion_id,
                value: segment.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        DnaMatchEventBody::SharedAncestorAsserted { ancestor, .. } => {
            state.shared_ancestors.push(Attributed {
                assertion_id,
                value: ancestor.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        DnaMatchEventBody::MatchConfirmed { .. } => {
            state.status = Some(Attributed {
                assertion_id,
                value: MatchStatus::Confirmed,
            });
            state.live_assertions.insert(assertion_id);
        }
        DnaMatchEventBody::MatchRejected { .. } => {
            state.status = Some(Attributed {
                assertion_id,
                value: MatchStatus::Rejected,
            });
            state.live_assertions.insert(assertion_id);
        }
        DnaMatchEventBody::NoteAttached { note_id, .. } => {
            state.notes.push(Attributed {
                assertion_id,
                value: *note_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        DnaMatchEventBody::Tagged { tag_id, .. } => {
            state.tags.push(Attributed {
                assertion_id,
                value: *tag_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        DnaMatchEventBody::Untagged { tag_id, .. } => {
            state.tags.retain(|t| t.value != *tag_id);
            state.live_assertions.insert(assertion_id);
        }
        DnaMatchEventBody::RestrictionsChanged { restrictions, .. } => {
            state.restrictions.clone_from(restrictions);
            state.live_assertions.insert(assertion_id);
        }
        DnaMatchEventBody::AssertionRetracted { target, .. }
        | DnaMatchEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::dna::{Centimorgans, DnaProvider};
    use crate::dna_match::command::DnaMatchCommand;
    use crate::dna_match::error::DnaMatchError;
    use crate::dna_match::ref_resolver::DnaMatchRefs;
    use crate::dna_match::state::{DnaMatchState, MatchStatus};
    use crate::ids::{AgentId, AssertionId, DnaMatchId, DnaTestId};
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use time::macros::datetime;
    use uuid::Uuid;

    const BOTH_PRESENT: DnaMatchRefs = DnaMatchRefs {
        test_a_exists: true,
        test_b_exists: true,
    };
    const A_MISSING: DnaMatchRefs = DnaMatchRefs {
        test_a_exists: false,
        test_b_exists: true,
    };

    fn dna_match(n: u128) -> DnaMatchId {
        DnaMatchId::from_uuid(Uuid::from_u128(n))
    }

    fn dna_test(n: u128) -> DnaTestId {
        DnaTestId::from_uuid(Uuid::from_u128(n))
    }

    fn meta(assertion: u128) -> AssertionMeta {
        AssertionMeta {
            assertion_id: AssertionId::from_uuid(Uuid::from_u128(assertion)),
            context: EventContext {
                operator: Agent {
                    kind: AgentKind::Human,
                    id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                    display: None,
                },
                occurred_at: Timestamp::new(datetime!(2026-06-19 12:00:00 UTC)),
                rationale: None,
                confidence: Confidence::Normal,
                citations: Vec::new(),
                evidence_analysis: None,
            },
        }
    }

    fn observe(test_a: DnaTestId, test_b: DnaTestId, shared_cm: Centimorgans) -> DnaMatchCommand {
        DnaMatchCommand::ObserveMatch {
            dna_match_id: dna_match(1),
            human_id: crate::ids::HumanId::new("X1"),
            test_a,
            test_b,
            provider: DnaProvider::MyHeritage,
            shared_cm,
            percent_shared: None,
            segment_count: 3,
            largest_segment_cm: Centimorgans::from_hundredths(4500),
            predicted_relationship: Some("2nd cousin".to_owned()),
        }
    }

    fn apply_all(state: &mut DnaMatchState, events: &[crate::dna_match::event::DnaMatchEvent]) {
        for event in events {
            evolve(state, event);
        }
    }

    #[test]
    fn same_test_on_both_sides_is_rejected() {
        let state = DnaMatchState::default();
        let err = decide(
            &state,
            observe(dna_test(1), dna_test(1), Centimorgans::from_hundredths(85_000)),
            &meta(1),
            &BOTH_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, DnaMatchError::SameTestBothSides(dna_test(1)));
    }

    #[test]
    fn a_missing_test_is_unknown_test() {
        let state = DnaMatchState::default();
        let err = decide(
            &state,
            observe(dna_test(1), dna_test(2), Centimorgans::from_hundredths(85_000)),
            &meta(1),
            &A_MISSING,
        )
        .unwrap_err();
        assert_eq!(err, DnaMatchError::UnknownTest(dna_test(1)));
    }

    #[test]
    fn negative_shared_cm_is_rejected() {
        let state = DnaMatchState::default();
        let err = decide(
            &state,
            observe(dna_test(1), dna_test(2), Centimorgans::from_hundredths(-1)),
            &meta(1),
            &BOTH_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, DnaMatchError::NegativeSharedCm);
    }

    #[test]
    fn note_and_tag_attach_project_and_retract() {
        use crate::ids::{NoteId, TagId};

        let mut state = DnaMatchState::default();
        let observed = decide(
            &state,
            observe(dna_test(1), dna_test(2), Centimorgans::from_hundredths(85_050)),
            &meta(1),
            &BOTH_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &observed);

        let note_id = NoteId::from_uuid(Uuid::from_u128(0x11));
        let tag_id = TagId::from_uuid(Uuid::from_u128(0x22));
        let attach = decide(
            &state,
            DnaMatchCommand::AttachNote {
                dna_match_id: dna_match(1),
                note_id,
            },
            &meta(2),
            &BOTH_PRESENT,
        )
        .unwrap();
        let note_assertion = attach[0].assertion_id;
        apply_all(&mut state, &attach);
        let tag = decide(
            &state,
            DnaMatchCommand::Tag {
                dna_match_id: dna_match(1),
                tag_id,
            },
            &meta(3),
            &BOTH_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &tag);

        assert_eq!(
            state.notes.iter().map(|n| n.value).collect::<Vec<_>>(),
            vec![note_id],
            "note is projected"
        );
        assert_eq!(
            state.tags.iter().map(|t| t.value).collect::<Vec<_>>(),
            vec![tag_id],
            "tag is projected"
        );

        let retract = decide(
            &state,
            DnaMatchCommand::RetractAssertion {
                dna_match_id: dna_match(1),
                target: note_assertion,
            },
            &meta(4),
            &BOTH_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.notes.is_empty(), "retracting clears the note");
        assert_eq!(
            state.tags.iter().map(|t| t.value).collect::<Vec<_>>(),
            vec![tag_id],
            "the tag is untouched"
        );
    }

    #[test]
    fn observe_then_confirm_records_status_and_shared_cm() {
        let mut state = DnaMatchState::default();
        let observed = decide(
            &state,
            observe(dna_test(1), dna_test(2), Centimorgans::from_hundredths(85_050)),
            &meta(1),
            &BOTH_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &observed);
        assert_eq!(state.shared_cm.map(Centimorgans::as_hundredths), Some(85_050));

        let confirmed = decide(
            &state,
            DnaMatchCommand::ConfirmMatch {
                dna_match_id: dna_match(1),
            },
            &meta(2),
            &BOTH_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &confirmed);
        assert_eq!(state.status.as_ref().map(|s| s.value), Some(MatchStatus::Confirmed));
    }
}
