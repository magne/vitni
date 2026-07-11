//! The pure `DnaTest` decision core (ADR 0004 §3) and the `evolve` fold.
//!
//! The cross-aggregate fact (does the anchoring person exist?) arrives in `refs`, resolved before
//! `decide` by the `Services`-backed adapter from the [`DnaTestRefResolver`](super::ref_resolver).

use crate::assertions::Attributed;
use crate::dna_test::command::DnaTestCommand;
use crate::dna_test::error::DnaTestError;
use crate::dna_test::event::{DnaTestEvent, DnaTestEventBody};
use crate::dna_test::ref_resolver::DnaTestRefs;
use crate::dna_test::state::DnaTestState;
use crate::ids::DnaTestId;
use crate::provenance::AssertionMeta;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`DnaTestError`] when the command violates an invariant: creating a test that exists,
/// a command against an absent test, anchoring to a person the projection does not know
/// (`refs.person_exists == false`), or correcting an unknown assertion.
pub fn decide(
    state: &DnaTestState,
    command: DnaTestCommand,
    meta: &AssertionMeta,
    refs: &DnaTestRefs,
) -> Result<Vec<DnaTestEvent>, DnaTestError> {
    match command {
        DnaTestCommand::CreateDnaTest {
            dna_test_id,
            human_id,
            person_id,
        } => {
            if state.exists {
                return Err(DnaTestError::AlreadyExists(dna_test_id));
            }
            if !refs.person_exists {
                return Err(DnaTestError::UnknownPerson(person_id));
            }
            Ok(one(
                meta,
                DnaTestEventBody::DnaTestCreated {
                    dna_test_id,
                    human_id,
                    person_id,
                },
            ))
        }
        // The plain commands share the same shape — exist-check then emit one event — so they
        // delegate to `simple_body` (exhaustive over them). Only `dna_test_id` is bound here (it is
        // `Copy`), leaving `command` intact to hand over.
        DnaTestCommand::SetProvider { dna_test_id, .. }
        | DnaTestCommand::SetKitId { dna_test_id, .. }
        | DnaTestCommand::SetTestType { dna_test_id, .. }
        | DnaTestCommand::SetGenomeBuild { dna_test_id, .. }
        | DnaTestCommand::AssertHaplogroup { dna_test_id, .. }
        | DnaTestCommand::AttachNote { dna_test_id, .. }
        | DnaTestCommand::Tag { dna_test_id, .. }
        | DnaTestCommand::Untag { dna_test_id, .. }
        | DnaTestCommand::SetRestrictions { dna_test_id, .. } => {
            ensure_exists(state, dna_test_id)?;
            Ok(one(meta, simple_body(command)))
        }
        DnaTestCommand::SetHumanId { dna_test_id, human_id } => {
            ensure_exists(state, dna_test_id)?;
            let old_human_id = state.human_id.clone().unwrap_or_else(|| human_id.clone());
            Ok(one(
                meta,
                DnaTestEventBody::HumanIdChanged {
                    dna_test_id,
                    human_id,
                    old_human_id,
                },
            ))
        }
        DnaTestCommand::RetractAssertion { dna_test_id, target } => {
            ensure_exists(state, dna_test_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(DnaTestError::RetractsMissingAssertion(target));
            }
            Ok(one(meta, DnaTestEventBody::AssertionRetracted { dna_test_id, target }))
        }
        DnaTestCommand::SupersedeAssertion {
            dna_test_id,
            target,
            replacement,
        } => {
            ensure_exists(state, dna_test_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(DnaTestError::SupersedesMissingAssertion(target));
            }
            let mut events = one(meta, DnaTestEventBody::AssertionSuperseded { dna_test_id, target });
            events.extend(decide(state, *replacement, meta, refs)?);
            Ok(events)
        }
    }
}

/// Maps a plain command to its event body (the existence check is done by `decide`).
///
/// Exhaustive over the plain commands; the lifecycle/cross-aggregate commands never reach here.
fn simple_body(command: DnaTestCommand) -> DnaTestEventBody {
    match command {
        DnaTestCommand::SetProvider { dna_test_id, provider } => {
            DnaTestEventBody::ProviderSet { dna_test_id, provider }
        }
        DnaTestCommand::SetKitId { dna_test_id, kit_id } => DnaTestEventBody::KitIdSet { dna_test_id, kit_id },
        DnaTestCommand::SetTestType { dna_test_id, test_type } => {
            DnaTestEventBody::TestTypeSet { dna_test_id, test_type }
        }
        DnaTestCommand::SetGenomeBuild {
            dna_test_id,
            genome_build,
        } => DnaTestEventBody::GenomeBuildSet {
            dna_test_id,
            genome_build,
        },
        DnaTestCommand::AssertHaplogroup {
            dna_test_id,
            haplogroup,
        } => DnaTestEventBody::HaplogroupAsserted {
            dna_test_id,
            haplogroup,
        },
        DnaTestCommand::AttachNote { dna_test_id, note_id } => DnaTestEventBody::NoteAttached { dna_test_id, note_id },
        DnaTestCommand::Tag { dna_test_id, tag_id } => DnaTestEventBody::Tagged { dna_test_id, tag_id },
        DnaTestCommand::Untag { dna_test_id, tag_id } => DnaTestEventBody::Untagged { dna_test_id, tag_id },
        DnaTestCommand::SetRestrictions {
            dna_test_id,
            restrictions,
        } => DnaTestEventBody::RestrictionsChanged {
            dna_test_id,
            restrictions,
        },
        DnaTestCommand::CreateDnaTest { .. }
        | DnaTestCommand::RetractAssertion { .. }
        | DnaTestCommand::SupersedeAssertion { .. }
        | DnaTestCommand::SetHumanId { .. } => unreachable!("handled by decide"),
    }
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: DnaTestEventBody) -> Vec<DnaTestEvent> {
    vec![DnaTestEvent::new(meta, body)]
}

/// Rejects a command that targets a test which has not been created yet.
fn ensure_exists(state: &DnaTestState, dna_test_id: DnaTestId) -> Result<(), DnaTestError> {
    if state.exists {
        Ok(())
    } else {
        Err(DnaTestError::NotFound(dna_test_id))
    }
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut DnaTestState, event: &DnaTestEvent) {
    let assertion_id = event.assertion_id;
    match &event.body {
        DnaTestEventBody::DnaTestCreated {
            dna_test_id,
            human_id,
            person_id,
        } => {
            state.exists = true;
            state.dna_test_id = Some(*dna_test_id);
            state.human_id = Some(human_id.clone());
            state.person_id = Some(*person_id);
            state.live_assertions.insert(assertion_id);
        }
        DnaTestEventBody::ProviderSet { provider, .. } => {
            state.provider = Some(Attributed {
                assertion_id,
                value: provider.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        DnaTestEventBody::KitIdSet { kit_id, .. } => {
            state.kit_id = Some(Attributed {
                assertion_id,
                value: kit_id.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        DnaTestEventBody::TestTypeSet { test_type, .. } => {
            state.test_type = Some(Attributed {
                assertion_id,
                value: *test_type,
            });
            state.live_assertions.insert(assertion_id);
        }
        DnaTestEventBody::GenomeBuildSet { genome_build, .. } => {
            state.genome_build = Some(Attributed {
                assertion_id,
                value: *genome_build,
            });
            state.live_assertions.insert(assertion_id);
        }
        DnaTestEventBody::HaplogroupAsserted { haplogroup, .. } => {
            state.haplogroups.push(Attributed {
                assertion_id,
                value: haplogroup.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        DnaTestEventBody::NoteAttached { note_id, .. } => {
            state.notes.push(Attributed {
                assertion_id,
                value: *note_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        DnaTestEventBody::Tagged { tag_id, .. } => {
            state.tags.push(Attributed {
                assertion_id,
                value: *tag_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        DnaTestEventBody::Untagged { tag_id, .. } => {
            state.tags.retain(|t| t.value != *tag_id);
            state.live_assertions.insert(assertion_id);
        }
        DnaTestEventBody::RestrictionsChanged { restrictions, .. } => {
            state.restrictions.clone_from(restrictions);
            state.live_assertions.insert(assertion_id);
        }
        DnaTestEventBody::HumanIdChanged { human_id, .. } => {
            state.human_id = Some(human_id.clone());
        }
        DnaTestEventBody::AssertionRetracted { target, .. } | DnaTestEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::dna::{DnaProvider, DnaTestType};
    use crate::dna_test::command::DnaTestCommand;
    use crate::dna_test::error::DnaTestError;
    use crate::dna_test::ref_resolver::DnaTestRefs;
    use crate::dna_test::state::DnaTestState;
    use crate::ids::{AgentId, AssertionId, DnaTestId, HumanId, PersonId};
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use time::macros::datetime;
    use uuid::Uuid;

    const PERSON_PRESENT: DnaTestRefs = DnaTestRefs { person_exists: true };
    const PERSON_MISSING: DnaTestRefs = DnaTestRefs { person_exists: false };

    fn test(n: u128) -> DnaTestId {
        DnaTestId::from_uuid(Uuid::from_u128(n))
    }

    fn person(n: u128) -> PersonId {
        PersonId::from_uuid(Uuid::from_u128(n))
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
                confidence: Some(Confidence::Normal),
                citations: Vec::new(),
                evidence_analysis: None,
            },
        }
    }

    fn apply_all(state: &mut DnaTestState, events: &[crate::dna_test::event::DnaTestEvent]) {
        for event in events {
            evolve(state, event);
        }
    }

    fn created_test(id: u128) -> DnaTestState {
        let mut state = DnaTestState::default();
        let events = decide(
            &state,
            DnaTestCommand::CreateDnaTest {
                dna_test_id: test(id),
                human_id: HumanId::new("D1"),
                person_id: person(1),
            },
            &meta(1),
            &PERSON_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &events);
        state
    }

    #[test]
    fn creating_for_a_missing_person_is_unknown_person() {
        let state = DnaTestState::default();
        let err = decide(
            &state,
            DnaTestCommand::CreateDnaTest {
                dna_test_id: test(1),
                human_id: HumanId::new("D1"),
                person_id: person(99),
            },
            &meta(1),
            &PERSON_MISSING,
        )
        .unwrap_err();
        assert_eq!(err, DnaTestError::UnknownPerson(person(99)));
    }

    #[test]
    fn note_and_tag_attach_project_and_retract() {
        use crate::ids::{NoteId, TagId};

        let mut state = created_test(1);
        let note_id = NoteId::from_uuid(Uuid::from_u128(0x11));
        let tag_id = TagId::from_uuid(Uuid::from_u128(0x22));

        let attach = decide(
            &state,
            DnaTestCommand::AttachNote {
                dna_test_id: test(1),
                note_id,
            },
            &meta(2),
            &PERSON_PRESENT,
        )
        .unwrap();
        let note_assertion = attach[0].assertion_id;
        apply_all(&mut state, &attach);
        let tag = decide(
            &state,
            DnaTestCommand::Tag {
                dna_test_id: test(1),
                tag_id,
            },
            &meta(3),
            &PERSON_PRESENT,
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
            DnaTestCommand::RetractAssertion {
                dna_test_id: test(1),
                target: note_assertion,
            },
            &meta(4),
            &PERSON_PRESENT,
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
    fn provider_is_last_writer_wins_and_haplogroups_accumulate() {
        let mut state = created_test(1);
        for command in [
            DnaTestCommand::SetProvider {
                dna_test_id: test(1),
                provider: DnaProvider::MyHeritage,
            },
            DnaTestCommand::SetTestType {
                dna_test_id: test(1),
                test_type: DnaTestType::Autosomal,
            },
            DnaTestCommand::AssertHaplogroup {
                dna_test_id: test(1),
                haplogroup: "R-M269".to_owned(),
            },
        ] {
            let events = decide(&state, command, &meta(2), &PERSON_PRESENT).unwrap();
            apply_all(&mut state, &events);
        }
        assert_eq!(
            state.provider.as_ref().map(|p| &p.value),
            Some(&DnaProvider::MyHeritage)
        );
        assert_eq!(state.haplogroups.len(), 1);
    }
}
