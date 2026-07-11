//! The pure Family decision core (ADR 0004 §3) and the `evolve` fold.
//!
//! [`decide`] is `decide(state, command, meta) -> Result<Vec<FamilyEvent>, FamilyError>`: it reads
//! no clock and generates no id (those arrive in `meta`), so it is unit-testable given/when/then
//! with no I/O. [`evolve`] applies an event to the state. Together they are the framework-agnostic
//! kernel the `cqrs-es` adapter wraps (ADR 0002).

use crate::assertions::{Asserted, Attributed};
use crate::family::command::FamilyCommand;
use crate::family::error::FamilyError;
use crate::family::event::{FamilyEvent, FamilyEventBody};
use crate::family::state::{AssertedChild, AssertedFamilyEvent, AssertedPartner, ChildRelationship, FamilyState};
use crate::ids::{CitationId, FamilyId};
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
        FamilyCommand::AddChild { family_id, child_id } => {
            ensure_exists(state, family_id)?;
            if state.has_child(child_id) {
                return Err(FamilyError::ChildAlreadyPresent(child_id));
            }
            FamilyEventBody::ChildAdded { family_id, child_id }
        }
        FamilyCommand::AssertChildRelationship {
            family_id,
            child_id,
            parent_id,
            relationship,
        } => child_relationship_body(state, family_id, child_id, parent_id, relationship)?,
        FamilyCommand::RemoveChild { family_id, child_id } => {
            ensure_exists(state, family_id)?;
            if !state.has_child(child_id) {
                return Err(FamilyError::ChildNotPresent(child_id));
            }
            FamilyEventBody::ChildRemoved { family_id, child_id }
        }
        FamilyCommand::SetRestrictions {
            family_id,
            restrictions,
        } => {
            ensure_exists(state, family_id)?;
            FamilyEventBody::RestrictionsChanged {
                family_id,
                restrictions,
            }
        }
        FamilyCommand::SetHumanId { family_id, human_id } => {
            ensure_exists(state, family_id)?;
            let old_human_id = state.human_id.clone().unwrap_or_else(|| human_id.clone());
            FamilyEventBody::HumanIdChanged {
                family_id,
                human_id,
                old_human_id,
            }
        }
        FamilyCommand::AddCitation { family_id, citation_id } => {
            ensure_exists(state, family_id)?;
            FamilyEventBody::CitationAdded { family_id, citation_id }
        }
        FamilyCommand::LinkFamilyEvent { family_id, event_id } => {
            ensure_exists(state, family_id)?;
            FamilyEventBody::FamilyEventLinked { family_id, event_id }
        }
        FamilyCommand::AttachMedia { family_id, media } => {
            ensure_exists(state, family_id)?;
            FamilyEventBody::MediaAttached { family_id, media }
        }
        FamilyCommand::AttachNote { family_id, note_id } => {
            ensure_exists(state, family_id)?;
            FamilyEventBody::NoteAttached { family_id, note_id }
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

/// Validates and builds the `ChildRelationshipAsserted` body: the child must be a member, the parent
/// a current partner, and the live `(child, parent)` pair unique (ADR 0021).
fn child_relationship_body(
    state: &FamilyState,
    family_id: FamilyId,
    child_id: crate::ids::PersonId,
    parent_id: crate::ids::PersonId,
    relationship: crate::enums::ChildParentRelationship,
) -> Result<FamilyEventBody, FamilyError> {
    ensure_exists(state, family_id)?;
    if !state.has_child(child_id) {
        return Err(FamilyError::ChildNotPresent(child_id));
    }
    if !state.has_partner(parent_id) {
        return Err(FamilyError::ParentNotPartner(parent_id));
    }
    if state.has_child_relationship(child_id, parent_id) {
        return Err(FamilyError::ChildRelationshipAlreadyPresent(child_id, parent_id));
    }
    Ok(FamilyEventBody::ChildRelationshipAsserted {
        family_id,
        child_id,
        parent_id,
        relationship,
    })
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: FamilyEventBody) -> Vec<FamilyEvent> {
    vec![FamilyEvent::new(meta, body)]
}

/// The backing citation ids an event carries in its provenance envelope (denormalized at fold time).
fn citation_ids(event: &FamilyEvent) -> Vec<CitationId> {
    event.context.citations.iter().map(|c| c.citation_id).collect()
}

/// Rejects a command that targets a family which has not been created yet.
fn ensure_exists(state: &FamilyState, family_id: FamilyId) -> Result<(), FamilyError> {
    if state.exists {
        Ok(())
    } else {
        Err(FamilyError::NotFound(family_id))
    }
}

/// Folds a `ChildRelationshipAsserted` into a live relationship row, denormalizing the envelope
/// provenance (surety + citations) like every other `Asserted` value (ADR 0021).
fn fold_child_relationship(state: &mut FamilyState, assertion_id: crate::ids::AssertionId, event: &FamilyEvent) {
    let FamilyEventBody::ChildRelationshipAsserted {
        child_id,
        parent_id,
        relationship,
        ..
    } = &event.body
    else {
        return;
    };
    let value = Asserted::from_context(
        ChildRelationship {
            child_id: *child_id,
            parent_id: *parent_id,
            relationship: relationship.clone(),
        },
        &event.context,
    );
    state.child_relationships.push(Attributed { assertion_id, value });
    state.live_assertions.insert(assertion_id);
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
            let value = AssertedPartner {
                person_id: *person_id,
                confidence: event.context.confidence,
                citations: citation_ids(event),
            };
            state.partners.push(Attributed { assertion_id, value });
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::ChildAdded { child_id, .. } => {
            let value = AssertedChild {
                child_id: *child_id,
                confidence: event.context.confidence,
                citations: citation_ids(event),
            };
            state.children.push(Attributed { assertion_id, value });
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::ChildRelationshipAsserted { .. } => fold_child_relationship(state, assertion_id, event),
        FamilyEventBody::PartnerRemoved { person_id, .. } => {
            state.partners.retain(|p| p.value.person_id != *person_id);
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::ChildRemoved { child_id, .. } => {
            state.children.retain(|c| c.value.child_id != *child_id);
            state.remove_child_rows(*child_id);
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::RestrictionsChanged { restrictions, .. } => {
            state.restrictions.clone_from(restrictions);
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::HumanIdChanged { human_id, .. } => {
            state.human_id = Some(human_id.clone());
        }
        FamilyEventBody::CitationAdded { citation_id, .. } => {
            state.citations.push(Attributed {
                assertion_id,
                value: *citation_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::FamilyEventLinked { event_id, .. } => {
            let value = AssertedFamilyEvent {
                event_id: *event_id,
                confidence: event.context.confidence,
                citations: citation_ids(event),
            };
            state.linked_events.push(Attributed { assertion_id, value });
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::MediaAttached { media, .. } => {
            state.media.push(Attributed {
                assertion_id,
                value: media.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::NoteAttached { note_id, .. } => {
            state.notes.push(Attributed {
                assertion_id,
                value: *note_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::Tagged { tag_id, .. } => {
            state.tags.push(Attributed {
                assertion_id,
                value: *tag_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        FamilyEventBody::Untagged { tag_id, .. } => {
            state.tags.retain(|t| t.value != *tag_id);
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
    fn attachments_project_into_state_and_a_retraction_removes_the_matching_one() {
        use crate::ids::{CitationId, MediaId, NoteId};
        use crate::text::MediaRef;

        let mut state = created_family(100);
        let media = MediaRef {
            media_id: MediaId::from_uuid(Uuid::from_u128(0x111)),
            crop: None,
            caption: None,
            citations: Vec::new(),
        };
        let attach_media = decide(
            &state,
            FamilyCommand::AttachMedia {
                family_id: fid(100),
                media,
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &attach_media);
        let citation = decide(
            &state,
            FamilyCommand::AddCitation {
                family_id: fid(100),
                citation_id: CitationId::from_uuid(Uuid::from_u128(0x222)),
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &citation);
        let note = decide(
            &state,
            FamilyCommand::AttachNote {
                family_id: fid(100),
                note_id: NoteId::from_uuid(Uuid::from_u128(0x333)),
            },
            &meta(4),
        )
        .unwrap();
        apply_all(&mut state, &note);

        assert_eq!(state.media.len(), 1);
        assert_eq!(state.citations.len(), 1);
        assert_eq!(state.notes.len(), 1);

        let retract = decide(
            &state,
            FamilyCommand::RetractAssertion {
                family_id: fid(100),
                target: AssertionId::from_uuid(Uuid::from_u128(3)),
            },
            &meta(5),
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.citations.is_empty(), "the citation assertion was retracted");
        assert_eq!(state.media.len(), 1, "other attachments are untouched");
        assert_eq!(state.notes.len(), 1);
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
        assert_eq!(state.partners[0].value.person_id, pid(1));
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

    /// A created family (100) with two partners (pid 1 asserted by 2, pid 2 by 3) and a child (pid 9,
    /// membership asserted by 4) — the fixture the per-link relationship tests build on.
    fn family_with_partners_and_child() -> FamilyState {
        let mut state = created_family(100);
        for (person, assertion) in [(pid(1), 2), (pid(2), 3)] {
            let add = decide(
                &state,
                FamilyCommand::AddPartner {
                    family_id: fid(100),
                    person_id: person,
                },
                &meta(assertion),
            )
            .unwrap();
            apply_all(&mut state, &add);
        }
        let add = decide(
            &state,
            FamilyCommand::AddChild {
                family_id: fid(100),
                child_id: pid(9),
            },
            &meta(4),
        )
        .unwrap();
        apply_all(&mut state, &add);
        state
    }

    /// Asserts `child`'s relationship to `parent`, folding the events into `state`.
    fn assert_relationship(
        state: &mut FamilyState,
        child: PersonId,
        parent: PersonId,
        kind: ChildParentRelationship,
        assertion: u128,
    ) {
        let events = decide(
            state,
            FamilyCommand::AssertChildRelationship {
                family_id: fid(100),
                child_id: child,
                parent_id: parent,
                relationship: kind,
            },
            &meta(assertion),
        )
        .unwrap();
        apply_all(state, &events);
    }

    #[test]
    fn add_child_carries_membership_only() {
        let mut state = created_family(100);
        let add = decide(
            &state,
            FamilyCommand::AddChild {
                family_id: fid(100),
                child_id: pid(2),
            },
            &meta(2),
        )
        .unwrap();
        assert_eq!(add.len(), 1);
        assert!(matches!(add[0].body, FamilyEventBody::ChildAdded { .. }));
        apply_all(&mut state, &add);
        assert_eq!(state.children.len(), 1);
        assert_eq!(state.children[0].value.child_id, pid(2));
        assert!(
            state.child_relationships.is_empty(),
            "membership carries no relationships"
        );
    }

    #[test]
    fn adding_then_removing_a_child_leaves_no_child() {
        let mut state = created_family(100);
        let add = decide(
            &state,
            FamilyCommand::AddChild {
                family_id: fid(100),
                child_id: pid(2),
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
            },
            &meta(3),
        )
        .unwrap_err();
        assert_eq!(err, FamilyError::ChildAlreadyPresent(pid(2)));
    }

    #[test]
    fn asserting_a_child_relationship_folds_its_own_row() {
        let mut state = family_with_partners_and_child();
        assert_relationship(&mut state, pid(9), pid(1), ChildParentRelationship::Birth, 5);
        assert_eq!(state.child_relationships.len(), 1);
        let row = &state.child_relationships[0];
        assert_eq!(row.assertion_id, AssertionId::from_uuid(Uuid::from_u128(5)));
        assert_eq!(row.value.value.child_id, pid(9));
        assert_eq!(row.value.value.parent_id, pid(1));
        assert_eq!(row.value.value.relationship, ChildParentRelationship::Birth);
        // The link denormalizes the envelope provenance (confidence + citations) like every Asserted row.
        assert_eq!(row.value.confidence, Confidence::Normal);
    }

    #[test]
    fn a_child_relationship_requires_membership() {
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
        let err = decide(
            &state,
            FamilyCommand::AssertChildRelationship {
                family_id: fid(100),
                child_id: pid(9),
                parent_id: pid(1),
                relationship: ChildParentRelationship::Birth,
            },
            &meta(3),
        )
        .unwrap_err();
        assert_eq!(err, FamilyError::ChildNotPresent(pid(9)));
    }

    #[test]
    fn a_child_relationship_requires_a_current_partner() {
        let state = family_with_partners_and_child();
        let err = decide(
            &state,
            FamilyCommand::AssertChildRelationship {
                family_id: fid(100),
                child_id: pid(9),
                parent_id: pid(7),
                relationship: ChildParentRelationship::Birth,
            },
            &meta(5),
        )
        .unwrap_err();
        assert_eq!(err, FamilyError::ParentNotPartner(pid(7)));
    }

    #[test]
    fn a_duplicate_live_child_relationship_is_rejected() {
        let mut state = family_with_partners_and_child();
        assert_relationship(&mut state, pid(9), pid(1), ChildParentRelationship::Birth, 5);
        // A different partner is fine.
        assert_relationship(&mut state, pid(9), pid(2), ChildParentRelationship::Step, 6);
        assert_eq!(state.child_relationships.len(), 2);
        // The same (child, parent) pair a second time is rejected.
        let err = decide(
            &state,
            FamilyCommand::AssertChildRelationship {
                family_id: fid(100),
                child_id: pid(9),
                parent_id: pid(1),
                relationship: ChildParentRelationship::Adopted,
            },
            &meta(7),
        )
        .unwrap_err();
        assert_eq!(err, FamilyError::ChildRelationshipAlreadyPresent(pid(9), pid(1)));
    }

    #[test]
    fn retracting_one_parent_link_leaves_membership_and_the_other_link_live() {
        let mut state = family_with_partners_and_child();
        assert_relationship(&mut state, pid(9), pid(1), ChildParentRelationship::Birth, 5);
        assert_relationship(&mut state, pid(9), pid(2), ChildParentRelationship::Step, 6);

        // Retract only the link asserted by 5 (child–P1).
        let retract = decide(
            &state,
            FamilyCommand::RetractAssertion {
                family_id: fid(100),
                target: AssertionId::from_uuid(Uuid::from_u128(5)),
            },
            &meta(7),
        )
        .unwrap();
        apply_all(&mut state, &retract);

        // Membership stands, the other link stands, only the retracted link is gone.
        assert_eq!(state.children.len(), 1, "membership is untouched");
        assert_eq!(state.child_relationships.len(), 1, "only the retracted link is dropped");
        assert_eq!(state.child_relationships[0].value.value.parent_id, pid(2));
        assert!(
            !state
                .live_assertions
                .contains(&AssertionId::from_uuid(Uuid::from_u128(5)))
        );
        assert!(
            state
                .live_assertions
                .contains(&AssertionId::from_uuid(Uuid::from_u128(6)))
        );
    }

    #[test]
    fn removing_a_child_cascades_its_relationship_rows() {
        let mut state = family_with_partners_and_child();
        assert_relationship(&mut state, pid(9), pid(1), ChildParentRelationship::Birth, 5);
        assert_relationship(&mut state, pid(9), pid(2), ChildParentRelationship::Step, 6);

        let remove = decide(
            &state,
            FamilyCommand::RemoveChild {
                family_id: fid(100),
                child_id: pid(9),
            },
            &meta(7),
        )
        .unwrap();
        apply_all(&mut state, &remove);
        assert!(state.children.is_empty());
        assert!(
            state.child_relationships.is_empty(),
            "the child's links cascade with its removal"
        );
        for assertion in [5, 6] {
            assert!(
                !state
                    .live_assertions
                    .contains(&AssertionId::from_uuid(Uuid::from_u128(assertion))),
                "the cascaded link {assertion} is no longer live"
            );
        }
    }

    #[test]
    fn retracting_the_membership_assertion_cascades_the_links() {
        let mut state = family_with_partners_and_child();
        assert_relationship(&mut state, pid(9), pid(1), ChildParentRelationship::Birth, 5);
        assert_relationship(&mut state, pid(9), pid(2), ChildParentRelationship::Step, 6);

        // Retract the membership assertion (4) — its links cascade.
        let retract = decide(
            &state,
            FamilyCommand::RetractAssertion {
                family_id: fid(100),
                target: AssertionId::from_uuid(Uuid::from_u128(4)),
            },
            &meta(7),
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.children.is_empty(), "the membership is retracted");
        assert!(
            state.child_relationships.is_empty(),
            "its links cascade with the membership"
        );
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
    fn superseding_one_parent_link_replaces_only_that_link() {
        let mut state = family_with_partners_and_child();
        assert_relationship(&mut state, pid(9), pid(1), ChildParentRelationship::Birth, 5);
        assert_relationship(&mut state, pid(9), pid(2), ChildParentRelationship::Step, 6);
        let target = AssertionId::from_uuid(Uuid::from_u128(5));

        // Supersede the child–P1 link (Birth) with an Adopted link. The duplicate guard passes
        // because the target is retracted before the replacement is decided.
        let events = decide(
            &state,
            FamilyCommand::SupersedeAssertion {
                family_id: fid(100),
                target,
                replacement: Box::new(FamilyCommand::AssertChildRelationship {
                    family_id: fid(100),
                    child_id: pid(9),
                    parent_id: pid(1),
                    relationship: ChildParentRelationship::Adopted,
                }),
            },
            &meta(7),
        )
        .unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].body, FamilyEventBody::AssertionSuperseded { .. }));
        assert!(matches!(
            events[1].body,
            FamilyEventBody::ChildRelationshipAsserted { .. }
        ));

        apply_all(&mut state, &events);
        assert_eq!(state.children.len(), 1, "membership is untouched");
        assert_eq!(state.child_relationships.len(), 2, "still two links, one replaced");
        let p1 = state
            .child_relationships
            .iter()
            .find(|r| r.value.value.parent_id == pid(1))
            .unwrap();
        assert_eq!(p1.value.value.relationship, ChildParentRelationship::Adopted);
        assert!(!state.live_assertions.contains(&target));
    }

    #[test]
    fn linking_a_family_event_projects_into_state_and_retracts_cleanly() {
        use crate::ids::EventId;

        let mut state = created_family(100);
        let event_id = EventId::from_uuid(Uuid::from_u128(0x5555));
        let link = decide(
            &state,
            FamilyCommand::LinkFamilyEvent {
                family_id: fid(100),
                event_id,
            },
            &meta(2),
        )
        .unwrap();
        assert!(matches!(link[0].body, FamilyEventBody::FamilyEventLinked { .. }));
        apply_all(&mut state, &link);
        assert_eq!(state.linked_events.len(), 1);
        assert_eq!(state.linked_events[0].value.event_id, event_id);

        let retract = decide(
            &state,
            FamilyCommand::RetractAssertion {
                family_id: fid(100),
                target: AssertionId::from_uuid(Uuid::from_u128(2)),
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(
            state.linked_events.is_empty(),
            "the linked event assertion was retracted"
        );
    }

    #[test]
    fn a_child_carries_a_relationship_per_partner() {
        let mut state = family_with_partners_and_child();
        assert_relationship(&mut state, pid(9), pid(1), ChildParentRelationship::Birth, 5);
        assert_relationship(&mut state, pid(9), pid(2), ChildParentRelationship::Step, 6);
        // One membership row and one relationship row per partner (the view folds them into a tuple
        // list — see the family_children_are_reconstructed_with_relationships app test).
        assert_eq!(state.children.len(), 1);
        let rows: Vec<_> = state
            .child_relationships
            .iter()
            .map(|r| (r.value.value.parent_id, r.value.value.relationship.clone()))
            .collect();
        assert_eq!(
            rows,
            vec![
                (pid(1), ChildParentRelationship::Birth),
                (pid(2), ChildParentRelationship::Step),
            ]
        );
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
