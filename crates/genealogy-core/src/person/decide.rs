//! The pure Person decision core (ADR 0004 §3) and the `evolve` fold.
//!
//! [`decide`] is `decide(state, command, meta) -> Result<Vec<PersonEvent>, PersonError>`: it reads
//! no clock and generates no id (those arrive in `meta`), so it is unit-testable given/when/then
//! with no I/O. [`evolve`] applies an event to the state. Together they are the framework-agnostic
//! kernel the `cqrs-es` adapter wraps (ADR 0002).

use crate::assertions::{Asserted, Attributed};
use crate::ids::PersonId;
use crate::person::command::PersonCommand;
use crate::person::error::PersonError;
use crate::person::event::{PersonEvent, PersonEventBody};
use crate::person::state::{Association, Participation, PersonState};
use crate::provenance::AssertionMeta;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`PersonError`] when the command violates a within-aggregate invariant
/// (data-model §10.1) — e.g. creating a person that exists, naming with neither given nor
/// surname, or correcting an unknown assertion.
pub fn decide(
    state: &PersonState,
    command: PersonCommand,
    meta: &AssertionMeta,
) -> Result<Vec<PersonEvent>, PersonError> {
    match command {
        PersonCommand::CreatePerson {
            person_id,
            human_id,
            evidence_level,
        } => {
            if state.exists {
                return Err(PersonError::AlreadyExists(person_id));
            }
            Ok(one(
                meta,
                PersonEventBody::PersonCreated {
                    person_id,
                    human_id,
                    evidence_level,
                },
            ))
        }
        PersonCommand::RetractAssertion { person_id, target } => {
            ensure_exists(state, person_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(PersonError::RetractsMissingAssertion(target));
            }
            Ok(one(meta, PersonEventBody::AssertionRetracted { person_id, target }))
        }
        PersonCommand::SupersedeAssertion {
            person_id,
            target,
            replacement,
        } => {
            ensure_exists(state, person_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(PersonError::SupersedesMissingAssertion(target));
            }
            let mut events = one(meta, PersonEventBody::AssertionSuperseded { person_id, target });
            events.extend(decide(state, *replacement, meta)?);
            Ok(events)
        }
        PersonCommand::MergePersons { surviving, merged } => {
            ensure_exists(state, surviving)?;
            if surviving == merged {
                return Err(PersonError::MergeConflict {
                    surviving,
                    merged,
                    reason: "a person cannot be merged with itself".to_owned(),
                });
            }
            Ok(one(meta, PersonEventBody::PersonsMerged { surviving, merged }))
        }
        assertion => decide_assertion(state, assertion, meta),
    }
}

/// Decides the plain assertion commands — those that simply require the person to exist and
/// (for a few) pass a small within-aggregate check before emitting one event.
fn decide_assertion(
    state: &PersonState,
    command: PersonCommand,
    meta: &AssertionMeta,
) -> Result<Vec<PersonEvent>, PersonError> {
    let body = match command {
        PersonCommand::AssertName { person_id, name } => {
            ensure_exists(state, person_id)?;
            if name.is_empty() {
                return Err(PersonError::EmptyName);
            }
            PersonEventBody::NameAsserted { person_id, name }
        }
        PersonCommand::AssertSex { person_id, sex } => {
            ensure_exists(state, person_id)?;
            PersonEventBody::SexAsserted { person_id, sex }
        }
        PersonCommand::AssertFact { person_id, fact } => {
            ensure_exists(state, person_id)?;
            PersonEventBody::FactAsserted { person_id, fact }
        }
        PersonCommand::AssertParticipation {
            person_id,
            event_id,
            role,
            age,
            attributes,
            notes,
        } => {
            ensure_exists(state, person_id)?;
            PersonEventBody::ParticipationAsserted {
                person_id,
                event_id,
                role,
                age,
                attributes,
                notes,
            }
        }
        PersonCommand::AssertAssociation { person_id, other, role } => {
            ensure_exists(state, person_id)?;
            if other == person_id {
                return Err(PersonError::SelfAssociation(person_id));
            }
            PersonEventBody::AssociationAsserted { person_id, other, role }
        }
        PersonCommand::AttachMedia { person_id, media } => {
            ensure_exists(state, person_id)?;
            PersonEventBody::MediaAttached { person_id, media }
        }
        PersonCommand::AttachNote { person_id, note_id } => {
            ensure_exists(state, person_id)?;
            PersonEventBody::NoteAttached { person_id, note_id }
        }
        PersonCommand::AddCitation { person_id, citation_id } => {
            ensure_exists(state, person_id)?;
            PersonEventBody::CitationAdded { person_id, citation_id }
        }
        PersonCommand::AddExternalId { person_id, external_id } => {
            ensure_exists(state, person_id)?;
            // Idempotent: re-adding the same identifier emits nothing, so re-import is a no-op.
            if state.has_external_id(&external_id.authority, &external_id.value) {
                return Ok(Vec::new());
            }
            PersonEventBody::ExternalIdAdded { person_id, external_id }
        }
        PersonCommand::Tag { person_id, tag_id } => {
            ensure_exists(state, person_id)?;
            PersonEventBody::Tagged { person_id, tag_id }
        }
        PersonCommand::Untag { person_id, tag_id } => {
            ensure_exists(state, person_id)?;
            PersonEventBody::Untagged { person_id, tag_id }
        }
        PersonCommand::SetRestrictions {
            person_id,
            restrictions,
        } => {
            ensure_exists(state, person_id)?;
            PersonEventBody::RestrictionsChanged {
                person_id,
                restrictions,
            }
        }
        PersonCommand::SetHumanId { person_id, human_id } => {
            ensure_exists(state, person_id)?;
            let old_human_id = state.human_id.clone().unwrap_or_else(|| human_id.clone());
            PersonEventBody::HumanIdChanged {
                person_id,
                human_id,
                old_human_id,
            }
        }
        // The lifecycle/correction commands are handled by `decide`; they never reach here.
        PersonCommand::CreatePerson { .. }
        | PersonCommand::RetractAssertion { .. }
        | PersonCommand::SupersedeAssertion { .. }
        | PersonCommand::MergePersons { .. } => unreachable!("handled by decide"),
    };
    Ok(one(meta, body))
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: PersonEventBody) -> Vec<PersonEvent> {
    vec![PersonEvent::new(meta, body)]
}

/// Rejects a command that targets a person which has not been created yet.
fn ensure_exists(state: &PersonState, person_id: PersonId) -> Result<(), PersonError> {
    if state.exists {
        Ok(())
    } else {
        Err(PersonError::NotFound(person_id))
    }
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut PersonState, event: &PersonEvent) {
    let assertion_id = event.assertion_id;
    match &event.body {
        PersonEventBody::PersonCreated {
            person_id,
            human_id,
            evidence_level,
        } => {
            state.exists = true;
            state.person_id = Some(*person_id);
            state.human_id = Some(human_id.clone());
            state.evidence_level = Some(*evidence_level);
            state.live_assertions.insert(assertion_id);
        }
        PersonEventBody::NameAsserted { name, .. } => {
            state.names.push(Attributed {
                assertion_id,
                value: Asserted::from_context(name.clone(), &event.context),
            });
            state.live_assertions.insert(assertion_id);
        }
        PersonEventBody::SexAsserted { sex, .. } => {
            state.sex.push(Attributed {
                assertion_id,
                value: Asserted::from_context(sex.clone(), &event.context),
            });
            state.live_assertions.insert(assertion_id);
        }
        PersonEventBody::FactAsserted { fact, .. } => {
            state.facts.push(Attributed {
                assertion_id,
                value: Asserted::from_context(fact.clone(), &event.context),
            });
            state.live_assertions.insert(assertion_id);
        }
        PersonEventBody::AssociationAsserted { other, role, .. } => {
            state.associations.push(Attributed {
                assertion_id,
                value: Asserted::from_context(
                    Association {
                        other: *other,
                        role: role.clone(),
                    },
                    &event.context,
                ),
            });
            state.live_assertions.insert(assertion_id);
        }
        PersonEventBody::ParticipationAsserted { .. } => fold_participation(state, assertion_id, event),
        PersonEventBody::ExternalIdAdded { external_id, .. } => {
            state.external_ids.push(Attributed {
                assertion_id,
                value: external_id.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        PersonEventBody::RestrictionsChanged { restrictions, .. } => {
            state.restrictions.clone_from(restrictions);
            state.restrictions_assertion = Some(assertion_id);
            state.live_assertions.insert(assertion_id);
        }
        PersonEventBody::HumanIdChanged { human_id, .. } => {
            state.human_id = Some(human_id.clone());
        }
        PersonEventBody::PersonsMerged { merged, .. } => {
            state.merged.push(Attributed {
                assertion_id,
                value: *merged,
            });
            state.live_assertions.insert(assertion_id);
        }
        PersonEventBody::AssertionRetracted { target, .. } | PersonEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
        }
        PersonEventBody::CitationAdded { .. }
        | PersonEventBody::MediaAttached { .. }
        | PersonEventBody::NoteAttached { .. }
        | PersonEventBody::Tagged { .. }
        | PersonEventBody::Untagged { .. } => {
            fold_attachment(state, assertion_id, &event.body);
            state.live_assertions.insert(assertion_id);
        }
    }
}

/// Folds a `ParticipationAsserted` into the projected state, denormalizing the envelope provenance
/// (surety + backing citations) onto the participation row (ADR 0019, ADR 0020).
fn fold_participation(state: &mut PersonState, assertion_id: crate::ids::AssertionId, event: &PersonEvent) {
    let PersonEventBody::ParticipationAsserted {
        event_id,
        role,
        age,
        attributes,
        notes,
        ..
    } = &event.body
    else {
        return;
    };
    state.participations.push(Attributed {
        assertion_id,
        value: Asserted::from_context(
            Participation {
                event_id: *event_id,
                role: role.clone(),
                age: age.clone(),
                attributes: attributes.clone(),
                notes: notes.clone(),
            },
            &event.context,
        ),
    });
    state.live_assertions.insert(assertion_id);
}

/// Folds an attachment event (citation/media/note/tag) into the projected state.
fn fold_attachment(state: &mut PersonState, assertion_id: crate::ids::AssertionId, body: &PersonEventBody) {
    match body {
        PersonEventBody::CitationAdded { citation_id, .. } => state.citations.push(Attributed {
            assertion_id,
            value: *citation_id,
        }),
        PersonEventBody::MediaAttached { media, .. } => state.media.push(Attributed {
            assertion_id,
            value: media.clone(),
        }),
        PersonEventBody::NoteAttached { note_id, .. } => state.notes.push(Attributed {
            assertion_id,
            value: *note_id,
        }),
        PersonEventBody::Tagged { tag_id, .. } => state.tags.push(Attributed {
            assertion_id,
            value: *tag_id,
        }),
        PersonEventBody::Untagged { tag_id, .. } => state.tags.retain(|t| t.value != *tag_id),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::enums::{AssociationRole, EvidenceLevel, Restriction, Sex};
    use crate::ids::{AgentId, AssertionId, HumanId, PersonId};
    use crate::name::{NameType, PersonName, Surname};
    use crate::person::command::PersonCommand;
    use crate::person::error::PersonError;
    use crate::person::event::PersonEventBody;
    use crate::person::state::PersonState;
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use crate::text::ExternalId;
    use std::collections::BTreeSet;
    use time::macros::datetime;
    use uuid::Uuid;

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
                confidence: Some(Confidence::Normal),
                citations: Vec::new(),
                evidence_analysis: None,
            },
        }
    }

    fn full_name(given: Option<&str>, surname: Option<&str>) -> PersonName {
        PersonName {
            name_type: NameType::BirthName,
            given: given.map(ToOwned::to_owned),
            surnames: surname
                .map(|s| {
                    vec![Surname {
                        prefix: None,
                        surname: s.to_owned(),
                        primary: true,
                        connector: None,
                    }]
                })
                .unwrap_or_default(),
            suffix: None,
            title: None,
            nickname: None,
            call_name: None,
            date: None,
            language: None,
            transliterations: Vec::new(),
        }
    }

    /// Folds a command's events into a fresh state — the application-layer load→decide→apply loop.
    fn apply_all(state: &mut PersonState, events: &[crate::person::event::PersonEvent]) {
        for event in events {
            evolve(state, event);
        }
    }

    fn created_person(person: u128) -> PersonState {
        let mut state = PersonState::default();
        let events = decide(
            &state,
            PersonCommand::CreatePerson {
                person_id: pid(person),
                human_id: HumanId::new("I1"),
                evidence_level: EvidenceLevel::Conclusion,
            },
            &meta(1),
        )
        .unwrap();
        apply_all(&mut state, &events);
        state
    }

    #[test]
    fn set_human_id_emits_human_id_changed_carrying_old_and_new() {
        let state = created_person(1);
        let events = decide(
            &state,
            PersonCommand::SetHumanId {
                person_id: pid(1),
                human_id: HumanId::new("I0042"),
            },
            &meta(2),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        match &events[0].body {
            PersonEventBody::HumanIdChanged {
                human_id, old_human_id, ..
            } => {
                assert_eq!(human_id.as_str(), "I0042");
                assert_eq!(old_human_id.as_str(), "I1");
            }
            other => panic!("expected HumanIdChanged, got {other:?}"),
        }
        assert_eq!(events[0].context.confidence, Some(Confidence::Normal));
    }

    #[test]
    fn set_human_id_projects_the_new_id() {
        let mut state = created_person(1);
        let events = decide(
            &state,
            PersonCommand::SetHumanId {
                person_id: pid(1),
                human_id: HumanId::new("I0042"),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &events);
        assert_eq!(state.human_id.as_ref().map(HumanId::as_str), Some("I0042"));
    }

    #[test]
    fn set_human_id_on_absent_person_is_not_found() {
        let state = PersonState::default();
        let err = decide(
            &state,
            PersonCommand::SetHumanId {
                person_id: pid(9),
                human_id: HumanId::new("I0042"),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, PersonError::NotFound(pid(9)));
    }

    #[test]
    fn create_person_on_empty_state_emits_person_created() {
        let state = PersonState::default();
        let events = decide(
            &state,
            PersonCommand::CreatePerson {
                person_id: pid(100),
                human_id: HumanId::new("I1"),
                evidence_level: EvidenceLevel::Persona,
            },
            &meta(1),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].body, PersonEventBody::PersonCreated { .. }));
    }

    #[test]
    fn recreating_an_existing_person_is_rejected() {
        let state = created_person(100);
        let err = decide(
            &state,
            PersonCommand::CreatePerson {
                person_id: pid(100),
                human_id: HumanId::new("I1"),
                evidence_level: EvidenceLevel::Conclusion,
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, PersonError::AlreadyExists(pid(100)));
    }

    #[test]
    fn command_against_absent_person_is_not_found() {
        let state = PersonState::default();
        let err = decide(
            &state,
            PersonCommand::AssertSex {
                person_id: pid(7),
                sex: Sex::Female,
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, PersonError::NotFound(pid(7)));
    }

    #[test]
    fn asserting_an_empty_name_is_rejected() {
        let state = created_person(100);
        let err = decide(
            &state,
            PersonCommand::AssertName {
                person_id: pid(100),
                name: full_name(None, None),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, PersonError::EmptyName);
    }

    #[test]
    fn asserting_a_valid_name_emits_name_asserted_carrying_the_meta() {
        let state = created_person(100);
        let m = meta(42);
        let events = decide(
            &state,
            PersonCommand::AssertName {
                person_id: pid(100),
                name: full_name(Some("Ada"), Some("Lovelace")),
            },
            &m,
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        // meta is copied verbatim onto the emitted event (ADR 0004 §3).
        assert_eq!(events[0].assertion_id, m.assertion_id);
        assert_eq!(events[0].context, m.context);
    }

    #[test]
    fn associating_a_person_with_itself_is_rejected() {
        let state = created_person(100);
        let err = decide(
            &state,
            PersonCommand::AssertAssociation {
                person_id: pid(100),
                other: pid(100),
                role: AssociationRole::Witness,
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, PersonError::SelfAssociation(pid(100)));
    }

    #[test]
    fn retracting_an_unknown_assertion_is_rejected() {
        let state = created_person(100);
        let unknown = AssertionId::from_uuid(Uuid::from_u128(999));
        let err = decide(
            &state,
            PersonCommand::RetractAssertion {
                person_id: pid(100),
                target: unknown,
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, PersonError::RetractsMissingAssertion(unknown));
    }

    #[test]
    fn retracting_a_live_assertion_removes_it_from_state_non_destructively() {
        // given: a created person with a name asserted by assertion 2.
        let mut state = created_person(100);
        let name_events = decide(
            &state,
            PersonCommand::AssertName {
                person_id: pid(100),
                name: full_name(Some("Ada"), None),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &name_events);
        let name_assertion = AssertionId::from_uuid(Uuid::from_u128(2));
        assert_eq!(state.names.len(), 1);
        assert!(state.live_assertions.contains(&name_assertion));

        // when: that assertion is retracted.
        let retract = decide(
            &state,
            PersonCommand::RetractAssertion {
                person_id: pid(100),
                target: name_assertion,
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &retract);

        // then: the derived name is gone and the assertion is no longer live.
        assert!(state.names.is_empty());
        assert!(!state.live_assertions.contains(&name_assertion));
    }

    #[test]
    fn retracting_a_restriction_change_clears_the_restrictions() {
        // given: a created person with restrictions set by assertion 2.
        let mut state = created_person(100);
        let set = decide(
            &state,
            PersonCommand::SetRestrictions {
                person_id: pid(100),
                restrictions: BTreeSet::from([Restriction::Locked]),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &set);
        let restriction_assertion = AssertionId::from_uuid(Uuid::from_u128(2));
        assert_eq!(state.restrictions, BTreeSet::from([Restriction::Locked]));

        // when: that assertion is retracted.
        let retract = decide(
            &state,
            PersonCommand::RetractAssertion {
                person_id: pid(100),
                target: restriction_assertion,
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &retract);

        // then: the restrictions are cleared and the assertion is no longer live.
        assert!(state.restrictions.is_empty(), "retracting the change clears the set");
        assert_eq!(state.restrictions_assertion, None);
        assert!(!state.live_assertions.contains(&restriction_assertion));
    }

    #[test]
    fn superseding_emits_a_supersession_then_the_replacement_event() {
        let mut state = created_person(100);
        let first = decide(
            &state,
            PersonCommand::AssertName {
                person_id: pid(100),
                name: full_name(Some("Ada"), None),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &first);
        let target = AssertionId::from_uuid(Uuid::from_u128(2));

        let events = decide(
            &state,
            PersonCommand::SupersedeAssertion {
                person_id: pid(100),
                target,
                replacement: Box::new(PersonCommand::AssertName {
                    person_id: pid(100),
                    name: full_name(Some("Augusta Ada"), Some("King")),
                }),
            },
            &meta(3),
        )
        .unwrap();

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].body, PersonEventBody::AssertionSuperseded { .. }));
        assert!(matches!(events[1].body, PersonEventBody::NameAsserted { .. }));

        apply_all(&mut state, &events);
        // the old name is gone, the replacement remains.
        assert_eq!(state.names.len(), 1);
        assert_eq!(state.names[0].value.value.given.as_deref(), Some("Augusta Ada"));
        assert!(!state.live_assertions.contains(&target));
    }

    #[test]
    fn merging_a_person_with_itself_is_a_conflict() {
        let state = created_person(100);
        let err = decide(
            &state,
            PersonCommand::MergePersons {
                surviving: pid(100),
                merged: pid(100),
            },
            &meta(2),
        )
        .unwrap_err();
        assert!(matches!(err, PersonError::MergeConflict { .. }));
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
        let mut state = created_person(100);
        let add = decide(
            &state,
            PersonCommand::AddExternalId {
                person_id: pid(100),
                external_id: external_id("ABC"),
            },
            &meta(2),
        )
        .unwrap();
        assert_eq!(add.len(), 1);
        apply_all(&mut state, &add);

        // re-import: the identical identifier produces no event.
        let again = decide(
            &state,
            PersonCommand::AddExternalId {
                person_id: pid(100),
                external_id: external_id("ABC"),
            },
            &meta(3),
        )
        .unwrap();
        assert!(again.is_empty());
    }

    #[test]
    fn retracting_an_external_id_removes_it_and_lets_it_be_re_added() {
        let mut state = created_person(100);
        let add = decide(
            &state,
            PersonCommand::AddExternalId {
                person_id: pid(100),
                external_id: external_id("ABC"),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &add);
        let assertion = AssertionId::from_uuid(Uuid::from_u128(2));

        let retract = decide(
            &state,
            PersonCommand::RetractAssertion {
                person_id: pid(100),
                target: assertion,
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.external_ids.is_empty());

        // once retracted, the same identifier is addable again (emits an event).
        let re_add = decide(
            &state,
            PersonCommand::AddExternalId {
                person_id: pid(100),
                external_id: external_id("ABC"),
            },
            &meta(4),
        )
        .unwrap();
        assert_eq!(re_add.len(), 1);
    }

    #[test]
    fn associations_accumulate_and_a_retraction_removes_the_matching_one() {
        let mut state = created_person(100);
        let assoc = decide(
            &state,
            PersonCommand::AssertAssociation {
                person_id: pid(100),
                other: pid(200),
                role: AssociationRole::Godparent,
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &assoc);
        assert_eq!(state.associations.len(), 1);
        assert_eq!(state.associations[0].value.value.other, pid(200));
        assert_eq!(state.associations[0].value.value.role, AssociationRole::Godparent);

        let target = AssertionId::from_uuid(Uuid::from_u128(2));
        let retract = decide(
            &state,
            PersonCommand::RetractAssertion {
                person_id: pid(100),
                target,
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.associations.is_empty());
    }

    #[test]
    fn attachments_project_into_state_and_a_retraction_removes_the_matching_one() {
        use crate::ids::{CitationId, MediaId, NoteId, TagId};
        use crate::text::MediaRef;

        let mut state = created_person(100);
        let media = MediaRef {
            media_id: MediaId::from_uuid(Uuid::from_u128(0x111)),
            crop: None,
            caption: None,
            citations: Vec::new(),
        };
        let attach_media = decide(
            &state,
            PersonCommand::AttachMedia {
                person_id: pid(100),
                media: media.clone(),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &attach_media);
        let citation = decide(
            &state,
            PersonCommand::AddCitation {
                person_id: pid(100),
                citation_id: CitationId::from_uuid(Uuid::from_u128(0x222)),
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &citation);
        let note = decide(
            &state,
            PersonCommand::AttachNote {
                person_id: pid(100),
                note_id: NoteId::from_uuid(Uuid::from_u128(0x333)),
            },
            &meta(4),
        )
        .unwrap();
        apply_all(&mut state, &note);
        let tag = decide(
            &state,
            PersonCommand::Tag {
                person_id: pid(100),
                tag_id: TagId::from_uuid(Uuid::from_u128(0x444)),
            },
            &meta(5),
        )
        .unwrap();
        apply_all(&mut state, &tag);

        assert_eq!(state.media.len(), 1);
        assert_eq!(state.citations.len(), 1);
        assert_eq!(state.notes.len(), 1);
        assert_eq!(state.tags.len(), 1);

        // Retracting the citation assertion (meta 3) removes only it.
        let retract = decide(
            &state,
            PersonCommand::RetractAssertion {
                person_id: pid(100),
                target: AssertionId::from_uuid(Uuid::from_u128(3)),
            },
            &meta(6),
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.citations.is_empty(), "the citation assertion was retracted");
        assert_eq!(state.media.len(), 1, "other attachments are untouched");
        assert_eq!(state.notes.len(), 1);
        assert_eq!(state.tags.len(), 1);
    }

    #[test]
    fn asserting_a_fact_records_its_assertion_confidence() {
        use crate::enums::FactType;
        use crate::fact::Fact;

        let mut state = created_person(100);
        let mut high = meta(2);
        high.context.confidence = Some(Confidence::High);
        let fact = Fact {
            fact_type: FactType::Occupation,
            date: None,
            place_id: None,
            value: Some("Carpenter".to_owned()),
        };
        let events = decide(
            &state,
            PersonCommand::AssertFact {
                person_id: pid(100),
                fact,
            },
            &high,
        )
        .unwrap();
        apply_all(&mut state, &events);

        assert_eq!(state.facts.len(), 1);
        assert_eq!(state.facts[0].value.value.fact_type, FactType::Occupation);
        assert_eq!(
            state.facts[0].value.confidence,
            Some(Confidence::High),
            "the fact carries the asserting operator's confidence from the envelope"
        );
    }

    #[test]
    fn untagging_removes_the_applied_tag() {
        use crate::ids::TagId;

        let mut state = created_person(100);
        let tag_id = TagId::from_uuid(Uuid::from_u128(0x444));
        let tag = decide(
            &state,
            PersonCommand::Tag {
                person_id: pid(100),
                tag_id,
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &tag);
        assert_eq!(state.tags.len(), 1);

        let untag = decide(
            &state,
            PersonCommand::Untag {
                person_id: pid(100),
                tag_id,
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &untag);
        assert!(state.tags.is_empty(), "untag removes the applied tag");
    }

    #[test]
    fn merging_two_distinct_persons_emits_persons_merged() {
        let state = created_person(100);
        let events = decide(
            &state,
            PersonCommand::MergePersons {
                surviving: pid(100),
                merged: pid(200),
            },
            &meta(2),
        )
        .unwrap();
        assert!(matches!(events[0].body, PersonEventBody::PersonsMerged { .. }));
    }

    #[test]
    fn merge_folds_into_state_and_undo_removes_it() {
        let mut state = created_person(100);
        let merge = decide(
            &state,
            PersonCommand::MergePersons {
                surviving: pid(100),
                merged: pid(200),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &merge);
        assert_eq!(state.merged.iter().map(|m| m.value).collect::<Vec<_>>(), vec![pid(200)]);

        let target = AssertionId::from_uuid(Uuid::from_u128(2));
        let retraction = decide(
            &state,
            PersonCommand::RetractAssertion {
                person_id: pid(100),
                target,
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &retraction);
        assert!(
            state.merged.is_empty(),
            "undoing the merge assertion removes the persona link: {:?}",
            state.merged
        );
        assert!(!state.live_assertions.contains(&target));
    }

    #[test]
    fn asserted_names_and_associations_denormalize_confidence_and_citations() {
        use crate::ids::CitationId;
        use crate::provenance::CitationRef;

        // A meta carrying High confidence and one backing citation in its EventContext.
        let mut sourced = meta(2);
        sourced.context.confidence = Some(Confidence::High);
        sourced.context.citations = vec![CitationRef {
            citation_id: CitationId::from_uuid(Uuid::from_u128(0xC1)),
        }];

        let mut state = created_person(1);
        let name = decide(
            &state,
            PersonCommand::AssertName {
                person_id: pid(1),
                name: full_name(Some("Ada"), Some("Lovelace")),
            },
            &sourced,
        )
        .unwrap();
        apply_all(&mut state, &name);
        let assoc = decide(
            &state,
            PersonCommand::AssertAssociation {
                person_id: pid(1),
                other: pid(200),
                role: AssociationRole::Godparent,
            },
            &sourced,
        )
        .unwrap();
        apply_all(&mut state, &assoc);

        // The fold copies the assertion's surety + backing-citation ids onto the projection.
        assert_eq!(state.names[0].value.confidence, Some(Confidence::High));
        assert_eq!(state.names[0].value.citations.len(), 1);
        assert_eq!(state.associations[0].value.confidence, Some(Confidence::High));
        assert_eq!(state.associations[0].value.citations.len(), 1);
    }

    #[test]
    fn asserted_facts_denormalize_confidence_and_citations() {
        use crate::enums::FactType;
        use crate::fact::Fact;
        use crate::ids::CitationId;
        use crate::provenance::CitationRef;

        // A meta carrying High confidence and one backing citation in its EventContext.
        let mut sourced = meta(2);
        sourced.context.confidence = Some(Confidence::High);
        sourced.context.citations = vec![CitationRef {
            citation_id: CitationId::from_uuid(Uuid::from_u128(0xC1)),
        }];

        let mut state = created_person(1);
        let fact = Fact {
            fact_type: FactType::Occupation,
            date: None,
            place_id: None,
            value: Some("Carpenter".to_owned()),
        };
        let events = decide(
            &state,
            PersonCommand::AssertFact {
                person_id: pid(1),
                fact,
            },
            &sourced,
        )
        .unwrap();
        apply_all(&mut state, &events);

        // The fold copies the assertion's surety + backing-citation ids from the envelope (ADR 0020).
        assert_eq!(state.facts[0].value.confidence, Some(Confidence::High));
        assert_eq!(
            state.facts[0].value.citations,
            vec![CitationId::from_uuid(Uuid::from_u128(0xC1))]
        );
    }

    #[test]
    fn asserted_name_with_no_confidence_folds_to_none() {
        // A meta whose envelope records no surety judgment (ADR 0021 §5).
        let mut unjudged = meta(2);
        unjudged.context.confidence = None;

        let mut state = created_person(1);
        let events = decide(
            &state,
            PersonCommand::AssertName {
                person_id: pid(1),
                name: full_name(Some("Ada"), Some("Lovelace")),
            },
            &unjudged,
        )
        .unwrap();
        apply_all(&mut state, &events);

        assert_eq!(state.names[0].value.confidence, None);
    }

    #[test]
    fn participation_asserted_carries_age_attributes_and_notes() {
        use crate::age::{Age, AgeBound};
        use crate::enums::ParticipantRole;
        use crate::ids::{EventId, NoteId};
        use crate::text::Attribute;
        use cqrs_es::DomainEvent;

        let state = created_person(1);
        let age = Age {
            bound: Some(AgeBound::GreaterThan),
            years: Some(42),
            months: Some(0),
            days: None,
            phrase: None,
        };
        let attributes = vec![Attribute {
            attribute_type: "occupation".to_owned(),
            value: "farmer".to_owned(),
        }];
        let notes = vec![NoteId::from_uuid(Uuid::from_u128(0xD1))];
        let events = decide(
            &state,
            PersonCommand::AssertParticipation {
                person_id: pid(1),
                event_id: EventId::from_uuid(Uuid::from_u128(0xE1)),
                role: ParticipantRole::Witness,
                age: Some(age.clone()),
                attributes: attributes.clone(),
                notes: notes.clone(),
            },
            &meta(2),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        let event = &events[0];
        match &event.body {
            PersonEventBody::ParticipationAsserted {
                age: body_age,
                attributes: body_attributes,
                notes: body_notes,
                ..
            } => {
                assert_eq!(body_age.as_ref(), Some(&age));
                assert_eq!(body_attributes, &attributes);
                assert_eq!(body_notes, &notes);
            }
            other => panic!("expected ParticipationAsserted, got {other:?}"),
        }
        assert_eq!(event.event_version(), "2.0");
        // New events decode: JSON round-trip through the envelope.
        let json = serde_json::to_string(event).expect("serialize");
        let back: crate::person::event::PersonEvent = serde_json::from_str(&json).expect("deserialize");
        assert_eq!(&back, event);
    }

    #[test]
    fn participations_denormalize_confidence_and_citations() {
        use crate::age::Age;
        use crate::enums::ParticipantRole;
        use crate::ids::{CitationId, EventId, NoteId};
        use crate::provenance::CitationRef;
        use crate::text::Attribute;

        let mut sourced = meta(2);
        sourced.context.confidence = Some(Confidence::High);
        sourced.context.citations = vec![CitationRef {
            citation_id: CitationId::from_uuid(Uuid::from_u128(0xC1)),
        }];

        let mut state = created_person(1);
        let events = decide(
            &state,
            PersonCommand::AssertParticipation {
                person_id: pid(1),
                event_id: EventId::from_uuid(Uuid::from_u128(0xE1)),
                role: ParticipantRole::Witness,
                age: Some(Age {
                    years: Some(30),
                    ..Age::default()
                }),
                attributes: vec![Attribute {
                    attribute_type: "residence".to_owned(),
                    value: "Bergen".to_owned(),
                }],
                notes: vec![NoteId::from_uuid(Uuid::from_u128(0xD1))],
            },
            &sourced,
        )
        .unwrap();
        apply_all(&mut state, &events);

        let row = &state.participations[0];
        let asserted = &row.value;
        assert_eq!(asserted.confidence, Some(Confidence::High));
        assert_eq!(asserted.citations, vec![CitationId::from_uuid(Uuid::from_u128(0xC1))]);
        assert_eq!(asserted.value.role, ParticipantRole::Witness);
        assert_eq!(asserted.value.age.as_ref().and_then(|a| a.years), Some(30));
        assert_eq!(asserted.value.attributes.len(), 1);
        assert_eq!(asserted.value.notes.len(), 1);
    }

    #[test]
    fn sex_assertion_denormalizes_confidence_and_citations() {
        use crate::ids::CitationId;
        use crate::provenance::CitationRef;

        let mut sourced = meta(2);
        sourced.context.confidence = Some(Confidence::High);
        sourced.context.citations = vec![CitationRef {
            citation_id: CitationId::from_uuid(Uuid::from_u128(0xC1)),
        }];

        let mut state = created_person(1);
        let events = decide(
            &state,
            PersonCommand::AssertSex {
                person_id: pid(1),
                sex: Sex::Female,
            },
            &sourced,
        )
        .unwrap();
        apply_all(&mut state, &events);

        // The sex row denormalizes the assertion's surety + backing citations like every Asserted value.
        assert_eq!(state.sex.len(), 1);
        assert_eq!(state.sex[0].value.value, Sex::Female);
        assert_eq!(state.sex[0].value.confidence, Some(Confidence::High));
        assert_eq!(
            state.sex[0].value.citations,
            vec![CitationId::from_uuid(Uuid::from_u128(0xC1))]
        );
    }

    #[test]
    fn superseding_sex_leaves_only_the_replacement_live() {
        let mut state = created_person(1);
        let first = decide(
            &state,
            PersonCommand::AssertSex {
                person_id: pid(1),
                sex: Sex::Female,
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &first);
        let target = AssertionId::from_uuid(Uuid::from_u128(2));

        let events = decide(
            &state,
            PersonCommand::SupersedeAssertion {
                person_id: pid(1),
                target,
                replacement: Box::new(PersonCommand::AssertSex {
                    person_id: pid(1),
                    sex: Sex::Male,
                }),
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &events);

        // Only the replacement is live; the last-wins read returns it.
        assert_eq!(state.sex.len(), 1);
        assert_eq!(state.sex.last().map(|s| s.value.value.clone()), Some(Sex::Male));
        assert!(!state.live_assertions.contains(&target));
    }

    #[test]
    fn retracting_the_latest_sex_restores_the_prior_assertion() {
        // given: two sex assertions — Female by 2, then Male by 3 (the later winning the read).
        let mut state = created_person(1);
        for (assertion, sex) in [(2, Sex::Female), (3, Sex::Male)] {
            let events = decide(
                &state,
                PersonCommand::AssertSex { person_id: pid(1), sex },
                &meta(assertion),
            )
            .unwrap();
            apply_all(&mut state, &events);
        }
        assert_eq!(state.sex.len(), 2);
        assert_eq!(state.sex.last().map(|s| s.value.value.clone()), Some(Sex::Male));

        // when: the latest assertion (3) is retracted.
        let retract = decide(
            &state,
            PersonCommand::RetractAssertion {
                person_id: pid(1),
                target: AssertionId::from_uuid(Uuid::from_u128(3)),
            },
            &meta(4),
        )
        .unwrap();
        apply_all(&mut state, &retract);

        // then: the prior assertion is restored as the live read, still carrying its provenance.
        assert_eq!(state.sex.len(), 1);
        let restored = &state.sex[0];
        assert_eq!(restored.assertion_id, AssertionId::from_uuid(Uuid::from_u128(2)));
        assert_eq!(restored.value.value, Sex::Female);
        assert_eq!(restored.value.confidence, Some(Confidence::Normal));
        assert!(
            state
                .live_assertions
                .contains(&AssertionId::from_uuid(Uuid::from_u128(2)))
        );
        assert!(
            !state
                .live_assertions
                .contains(&AssertionId::from_uuid(Uuid::from_u128(3)))
        );
    }
}
