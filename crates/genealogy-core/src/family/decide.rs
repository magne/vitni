//! The pure Family decision core (ADR 0004 §3) and the `evolve` fold.
//!
//! [`decide`] is `decide(state, command, meta) -> Result<Vec<FamilyEvent>, FamilyError>`: it reads
//! no clock and generates no id (those arrive in `meta`), so it is unit-testable given/when/then
//! with no I/O. [`evolve`] applies an event to the state. Together they are the framework-agnostic
//! kernel the `cqrs-es` adapter wraps (ADR 0002).

use crate::assertions::Attributed;
use crate::family::command::FamilyCommand;
use crate::family::error::FamilyError;
use crate::family::event::{FamilyEvent, FamilyEventBody};
use crate::family::state::{ChildEntry, FamilyState};
use crate::ids::FamilyId;
use crate::provenance::AssertionMeta;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`FamilyError`] when the command violates a within-aggregate invariant
/// (data-model §10.1) — e.g. creating a family that exists, removing a partner who is not present,
/// or correcting an unknown assertion.
pub fn decide(
    state: &FamilyState,
    command: FamilyCommand,
    meta: &AssertionMeta,
) -> Result<Vec<FamilyEvent>, FamilyError> {
    match command {
        FamilyCommand::CreateFamily { family_id, human_id } => {
            if state.exists {
                return Err(FamilyError::AlreadyExists(family_id));
            }
            Ok(one(meta, FamilyEventBody::FamilyCreated { family_id, human_id }))
        }
        FamilyCommand::RetractAssertion { family_id, target } => {
            ensure_exists(state, family_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(FamilyError::RetractsMissingAssertion(target));
            }
            Ok(one(meta, FamilyEventBody::AssertionRetracted { family_id, target }))
        }
        FamilyCommand::SupersedeAssertion {
            family_id,
            target,
            replacement,
        } => {
            ensure_exists(state, family_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(FamilyError::SupersedesMissingAssertion(target));
            }
            let mut events = one(meta, FamilyEventBody::AssertionSuperseded { family_id, target });
            // The replacement is asserted *after* the target is retracted, so a precondition like
            // the duplicate-child guard sees state without the superseded assertion.
            let mut post_supersession = state.clone();
            post_supersession.remove_assertion(target);
            events.extend(decide(&post_supersession, *replacement, meta)?);
            Ok(events)
        }
        assertion => decide_assertion(state, assertion, meta),
    }
}

/// Decides the membership/attribute commands — those that require the family to exist and pass a
/// small within-aggregate check before emitting one event.
fn decide_assertion(
    state: &FamilyState,
    command: FamilyCommand,
    meta: &AssertionMeta,
) -> Result<Vec<FamilyEvent>, FamilyError> {
    let body = match command {
        FamilyCommand::AddPartner { family_id, person_id } => {
            ensure_exists(state, family_id)?;
            if state.has_partner(person_id) {
                return Err(FamilyError::PartnerAlreadyPresent(person_id));
            }
            FamilyEventBody::PartnerAdded { family_id, person_id }
        }
        FamilyCommand::RemovePartner { family_id, person_id } => {
            ensure_exists(state, family_id)?;
            if !state.has_partner(person_id) {
                return Err(FamilyError::PartnerNotPresent(person_id));
            }
            FamilyEventBody::PartnerRemoved { family_id, person_id }
        }
        FamilyCommand::AddChild {
            family_id,
            child_id,
            relationship,
        } => {
            ensure_exists(state, family_id)?;
            if state.has_child(child_id) {
                return Err(FamilyError::ChildAlreadyPresent(child_id));
            }
            FamilyEventBody::ChildAdded {
                family_id,
                child_id,
                relationship,
            }
        }
        FamilyCommand::RemoveChild { family_id, child_id } => {
            ensure_exists(state, family_id)?;
            if !state.has_child(child_id) {
                return Err(FamilyError::ChildNotPresent(child_id));
            }
            FamilyEventBody::ChildRemoved { family_id, child_id }
        }
        FamilyCommand::SetPrivacy { family_id, private } => {
            ensure_exists(state, family_id)?;
            FamilyEventBody::PrivacyChanged { family_id, private }
        }
        FamilyCommand::Tag { family_id, tag_id } => {
            ensure_exists(state, family_id)?;
            FamilyEventBody::Tagged { family_id, tag_id }
        }
        FamilyCommand::Untag { family_id, tag_id } => {
            ensure_exists(state, family_id)?;
            FamilyEventBody::Untagged { family_id, tag_id }
        }
        FamilyCommand::AddExternalId { family_id, external_id } => {
            ensure_exists(state, family_id)?;
            // Idempotent: re-adding the same identifier emits nothing, so re-import is a no-op.
            if state.has_external_id(&external_id.authority, &external_id.value) {
                return Ok(Vec::new());
            }
            FamilyEventBody::ExternalIdAdded { family_id, external_id }
        }
        // The lifecycle/correction commands are handled by `decide`; they never reach here.
        FamilyCommand::CreateFamily { .. }
        | FamilyCommand::RetractAssertion { .. }
        | FamilyCommand::SupersedeAssertion { .. } => unreachable!("handled by decide"),
    };
    Ok(one(meta, body))
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: FamilyEventBody) -> Vec<FamilyEvent> {
    vec![FamilyEvent::new(meta, body)]
}

/// Rejects a command that targets a family which has not been created yet.
fn ensure_exists(state: &FamilyState, family_id: FamilyId) -> Result<(), FamilyError> {
    if state.exists {
        Ok(())
    } else {
        Err(FamilyError::NotFound(family_id))
    }
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut FamilyState, event: &FamilyEvent) {
    let assertion_id = event.assertion_id;
    match &event.body {
        FamilyEventBody::FamilyCreated { family_id, human_id } => {
            state.exists = true;
            state.family_id = Some(*family_id);
            state.human_id = Some(human_id.clone());
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::PartnerAdded { person_id, .. } => {
            state.partners.push(Attributed {
                assertion_id,
                value: *person_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::ChildAdded {
            child_id, relationship, ..
        } => {
            state.children.push(Attributed {
                assertion_id,
                value: ChildEntry {
                    child_id: *child_id,
                    relationship: relationship.clone(),
                },
            });
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::PartnerRemoved { person_id, .. } => {
            state.partners.retain(|p| p.value != *person_id);
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::ChildRemoved { child_id, .. } => {
            state.children.retain(|c| c.value.child_id != *child_id);
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::PrivacyChanged { private, .. } => {
            state.private = *private;
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::Tagged { .. } | FamilyEventBody::Untagged { .. } => {
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::ExternalIdAdded { external_id, .. } => {
            state.external_ids.push(Attributed {
                assertion_id,
                value: external_id.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::AssertionRetracted { target, .. } | FamilyEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::enums::ChildParentRelationship;
    use crate::family::command::FamilyCommand;
    use crate::family::error::FamilyError;
    use crate::family::event::{FamilyEvent, FamilyEventBody};
    use crate::family::state::FamilyState;
    use crate::ids::{AgentId, AssertionId, FamilyId, HumanId, PersonId};
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use crate::text::ExternalId;
    use time::macros::datetime;
    use uuid::Uuid;

    fn fid(n: u128) -> FamilyId {
        FamilyId::from_uuid(Uuid::from_u128(n))
    }

    fn external_id(value: &str) -> ExternalId {
        ExternalId {
            authority: "gedcom-uid".to_owned(),
            value: value.to_owned(),
            kind: None,
            url: None,
        }
    }

    #[test]
    fn adding_the_same_external_id_twice_emits_nothing() {
        let mut state = created_family(100);
        let add = decide(
            &state,
            FamilyCommand::AddExternalId {
                family_id: fid(100),
                external_id: external_id("F-UID"),
            },
            &meta(2),
        )
        .unwrap();
        assert_eq!(add.len(), 1);
        apply_all(&mut state, &add);

        let again = decide(
            &state,
            FamilyCommand::AddExternalId {
                family_id: fid(100),
                external_id: external_id("F-UID"),
            },
            &meta(3),
        )
        .unwrap();
        assert!(again.is_empty());
    }

    fn pid(n: u128) -> PersonId {
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
                occurred_at: Timestamp::new(datetime!(2026-06-17 12:00:00 UTC)),
                rationale: None,
                confidence: Confidence::Normal,
                citations: Vec::new(),
                evidence_analysis: None,
            },
        }
    }

    /// Folds a command's events into a fresh state — the application-layer load→decide→apply loop.
    fn apply_all(state: &mut FamilyState, events: &[FamilyEvent]) {
        for event in events {
            evolve(state, event);
        }
    }

    fn created_family(family: u128) -> FamilyState {
        let mut state = FamilyState::default();
        let events = decide(
            &state,
            FamilyCommand::CreateFamily {
                family_id: fid(family),
                human_id: HumanId::new("F1"),
            },
            &meta(1),
        )
        .unwrap();
        apply_all(&mut state, &events);
        state
    }

    #[test]
    fn create_family_on_empty_state_emits_family_created() {
        let state = FamilyState::default();
        let events = decide(
            &state,
            FamilyCommand::CreateFamily {
                family_id: fid(100),
                human_id: HumanId::new("F1"),
            },
            &meta(1),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].body, FamilyEventBody::FamilyCreated { .. }));
    }

    #[test]
    fn recreating_an_existing_family_is_rejected() {
        let state = created_family(100);
        let err = decide(
            &state,
            FamilyCommand::CreateFamily {
                family_id: fid(100),
                human_id: HumanId::new("F1"),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, FamilyError::AlreadyExists(fid(100)));
    }

    #[test]
    fn command_against_absent_family_is_not_found() {
        let state = FamilyState::default();
        let err = decide(
            &state,
            FamilyCommand::AddPartner {
                family_id: fid(7),
                person_id: pid(1),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, FamilyError::NotFound(fid(7)));
    }

    #[test]
    fn adding_a_partner_emits_partner_added_and_folds_into_state() {
        let mut state = created_family(100);
        let events = decide(
            &state,
            FamilyCommand::AddPartner {
                family_id: fid(100),
                person_id: pid(1),
            },
            &meta(2),
        )
        .unwrap();
        assert!(matches!(events[0].body, FamilyEventBody::PartnerAdded { .. }));
        apply_all(&mut state, &events);
        assert_eq!(state.partners.len(), 1);
        assert_eq!(state.partners[0].value, pid(1));
    }

    #[test]
    fn adding_the_same_partner_twice_is_rejected() {
        let mut state = created_family(100);
        let events = decide(
            &state,
            FamilyCommand::AddPartner {
                family_id: fid(100),
                person_id: pid(1),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &events);
        let err = decide(
            &state,
            FamilyCommand::AddPartner {
                family_id: fid(100),
                person_id: pid(1),
            },
            &meta(3),
        )
        .unwrap_err();
        assert_eq!(err, FamilyError::PartnerAlreadyPresent(pid(1)));
    }

    #[test]
    fn removing_an_absent_partner_is_rejected() {
        let state = created_family(100);
        let err = decide(
            &state,
            FamilyCommand::RemovePartner {
                family_id: fid(100),
                person_id: pid(1),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, FamilyError::PartnerNotPresent(pid(1)));
    }

    #[test]
    fn adding_then_removing_a_child_leaves_no_child() {
        let mut state = created_family(100);
        let add = decide(
            &state,
            FamilyCommand::AddChild {
                family_id: fid(100),
                child_id: pid(2),
                relationship: ChildParentRelationship::Birth,
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &add);
        assert_eq!(state.children.len(), 1);

        let remove = decide(
            &state,
            FamilyCommand::RemoveChild {
                family_id: fid(100),
                child_id: pid(2),
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &remove);
        assert!(state.children.is_empty());
    }

    #[test]
    fn adding_the_same_child_twice_is_rejected() {
        let mut state = created_family(100);
        let add = decide(
            &state,
            FamilyCommand::AddChild {
                family_id: fid(100),
                child_id: pid(2),
                relationship: ChildParentRelationship::Birth,
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &add);
        let err = decide(
            &state,
            FamilyCommand::AddChild {
                family_id: fid(100),
                child_id: pid(2),
                relationship: ChildParentRelationship::Adopted,
            },
            &meta(3),
        )
        .unwrap_err();
        assert_eq!(err, FamilyError::ChildAlreadyPresent(pid(2)));
    }

    #[test]
    fn retracting_a_partner_assertion_removes_it_non_destructively() {
        // given: a created family with a partner asserted by assertion 2.
        let mut state = created_family(100);
        let add = decide(
            &state,
            FamilyCommand::AddPartner {
                family_id: fid(100),
                person_id: pid(1),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &add);
        let partner_assertion = AssertionId::from_uuid(Uuid::from_u128(2));
        assert!(state.live_assertions.contains(&partner_assertion));

        // when: that assertion is retracted.
        let retract = decide(
            &state,
            FamilyCommand::RetractAssertion {
                family_id: fid(100),
                target: partner_assertion,
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &retract);

        // then: the partner is gone and the assertion is no longer live.
        assert!(state.partners.is_empty());
        assert!(!state.live_assertions.contains(&partner_assertion));
    }

    #[test]
    fn retracting_an_unknown_assertion_is_rejected() {
        let state = created_family(100);
        let unknown = AssertionId::from_uuid(Uuid::from_u128(999));
        let err = decide(
            &state,
            FamilyCommand::RetractAssertion {
                family_id: fid(100),
                target: unknown,
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, FamilyError::RetractsMissingAssertion(unknown));
    }

    #[test]
    fn superseding_emits_a_supersession_then_the_replacement_event() {
        let mut state = created_family(100);
        let first = decide(
            &state,
            FamilyCommand::AddChild {
                family_id: fid(100),
                child_id: pid(2),
                relationship: ChildParentRelationship::Birth,
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &first);
        let target = AssertionId::from_uuid(Uuid::from_u128(2));

        let events = decide(
            &state,
            FamilyCommand::SupersedeAssertion {
                family_id: fid(100),
                target,
                replacement: Box::new(FamilyCommand::AddChild {
                    family_id: fid(100),
                    child_id: pid(2),
                    relationship: ChildParentRelationship::Adopted,
                }),
            },
            &meta(3),
        )
        .unwrap();

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].body, FamilyEventBody::AssertionSuperseded { .. }));
        assert!(matches!(events[1].body, FamilyEventBody::ChildAdded { .. }));

        apply_all(&mut state, &events);
        // the old child entry is gone, the replacement remains with the new relationship.
        assert_eq!(state.children.len(), 1);
        assert_eq!(state.children[0].value.relationship, ChildParentRelationship::Adopted);
        assert!(!state.live_assertions.contains(&target));
    }

    #[test]
    fn asserting_carries_the_meta_onto_the_event() {
        let state = created_family(100);
        let m = meta(42);
        let events = decide(
            &state,
            FamilyCommand::AddPartner {
                family_id: fid(100),
                person_id: pid(1),
            },
            &m,
        )
        .unwrap();
        // meta is copied verbatim onto the emitted event (ADR 0004 §3).
        assert_eq!(events[0].assertion_id, m.assertion_id);
        assert_eq!(events[0].context, m.context);
    }
}
