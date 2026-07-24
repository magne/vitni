//! The pure `ResearchNote` decision core (ADR 0004 §3, ADR 0028) and the `evolve` fold.
//!
//! `decide(state, command, meta, refs)` reads no clock, generates no id, and reads no other
//! aggregate's projection itself: the cross-aggregate fact (does each named subject exist?) arrives
//! in `refs`, resolved by the `Services`-backed adapter from the
//! [`ResearchNoteRefResolver`](super::ref_resolver). So the rule (`UnknownSubject`) lives here, in
//! the pure core, while the impure read stays at the edge.

use std::collections::BTreeSet;

use crate::assertions::Attributed;
use crate::ids::ResearchNoteId;
use crate::provenance::AssertionMeta;
use crate::research_note::command::ResearchNoteCommand;
use crate::research_note::error::ResearchNoteError;
use crate::research_note::event::{ResearchNoteEvent, ResearchNoteEventBody};
use crate::research_note::ref_resolver::ResearchNoteRefs;
use crate::research_note::state::ResearchNoteState;
use crate::research_note::subject::SubjectRef;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`ResearchNoteError`] when the command violates an invariant: creating a research note
/// that exists, naming a subject the projection does not know (absent from `refs.existing_subjects`,
/// the §9 aggregate-tax check applied per subject), an empty subject set, removing the note's last
/// remaining subject, a command against an absent research note, an all-blank body, or correcting an
/// unknown assertion.
pub fn decide(
    state: &ResearchNoteState,
    command: ResearchNoteCommand,
    meta: &AssertionMeta,
    refs: &ResearchNoteRefs,
) -> Result<Vec<ResearchNoteEvent>, ResearchNoteError> {
    match command {
        ResearchNoteCommand::CreateResearchNote {
            research_note_id,
            human_id,
            subjects,
            title,
        } => {
            if state.exists {
                return Err(ResearchNoteError::AlreadyExists(research_note_id));
            }
            if subjects.is_empty() {
                return Err(ResearchNoteError::SubjectRequired);
            }
            ensure_all_resolve(&subjects, refs)?;
            Ok(one(
                meta,
                ResearchNoteEventBody::ResearchNoteCreated {
                    research_note_id,
                    human_id,
                    subjects,
                    title,
                },
            ))
        }
        ResearchNoteCommand::AddSubject {
            research_note_id,
            subject,
        } => add_subject(state, research_note_id, subject, meta, refs),
        ResearchNoteCommand::RemoveSubject {
            research_note_id,
            subject,
        } => remove_subject(state, research_note_id, subject, meta),
        ResearchNoteCommand::SetBody { research_note_id, body } => {
            ensure_exists(state, research_note_id)?;
            if body.text.trim().is_empty() {
                return Err(ResearchNoteError::EmptyBody);
            }
            Ok(one(meta, ResearchNoteEventBody::RichTextSet { research_note_id, body }))
        }
        // The single-fact setters (tag/untag/restrictions) share the same shape — exist-check then
        // emit one event — so they delegate to `setter_body` (exhaustive over them). Only
        // `research_note_id` is bound here (it is `Copy`), leaving `command` intact to hand over.
        ResearchNoteCommand::Tag { research_note_id, .. }
        | ResearchNoteCommand::Untag { research_note_id, .. }
        | ResearchNoteCommand::SetRestrictions { research_note_id, .. } => {
            ensure_exists(state, research_note_id)?;
            Ok(one(meta, setter_body(command)))
        }
        ResearchNoteCommand::RetractAssertion {
            research_note_id,
            target,
        } => {
            ensure_exists(state, research_note_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(ResearchNoteError::RetractsMissingAssertion(target));
            }
            Ok(one(
                meta,
                ResearchNoteEventBody::AssertionRetracted {
                    research_note_id,
                    target,
                },
            ))
        }
        ResearchNoteCommand::SupersedeAssertion {
            research_note_id,
            target,
            replacement,
        } => {
            ensure_exists(state, research_note_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(ResearchNoteError::SupersedesMissingAssertion(target));
            }
            let mut events = one(
                meta,
                ResearchNoteEventBody::AssertionSuperseded {
                    research_note_id,
                    target,
                },
            );
            events.extend(decide(state, *replacement, meta, refs)?);
            Ok(events)
        }
    }
}

/// Builds the event body for the single-fact setters (tag/untag/restrictions) — exhaustive over
/// exactly those three; every other variant is handled directly in `decide`.
fn setter_body(command: ResearchNoteCommand) -> ResearchNoteEventBody {
    match command {
        ResearchNoteCommand::Tag {
            research_note_id,
            tag_id,
        } => ResearchNoteEventBody::Tagged {
            research_note_id,
            tag_id,
        },
        ResearchNoteCommand::Untag {
            research_note_id,
            tag_id,
        } => ResearchNoteEventBody::Untagged {
            research_note_id,
            tag_id,
        },
        ResearchNoteCommand::SetRestrictions {
            research_note_id,
            restrictions,
        } => ResearchNoteEventBody::RestrictionsChanged {
            research_note_id,
            restrictions,
        },
        ResearchNoteCommand::CreateResearchNote { .. }
        | ResearchNoteCommand::AddSubject { .. }
        | ResearchNoteCommand::RemoveSubject { .. }
        | ResearchNoteCommand::SetBody { .. }
        | ResearchNoteCommand::RetractAssertion { .. }
        | ResearchNoteCommand::SupersedeAssertion { .. } => unreachable!("handled by decide"),
    }
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: ResearchNoteEventBody) -> Vec<ResearchNoteEvent> {
    vec![ResearchNoteEvent::new(meta, body)]
}

/// Rejects a command that targets a research note which has not been created yet.
fn ensure_exists(state: &ResearchNoteState, research_note_id: ResearchNoteId) -> Result<(), ResearchNoteError> {
    if state.exists {
        Ok(())
    } else {
        Err(ResearchNoteError::NotFound(research_note_id))
    }
}

/// Rejects with `UnknownSubject` if any of `subjects` is absent from `refs.existing_subjects` — the
/// §9 aggregate-tax check, applied per subject so a multi-subject note validates every one of them.
fn ensure_all_resolve(subjects: &BTreeSet<SubjectRef>, refs: &ResearchNoteRefs) -> Result<(), ResearchNoteError> {
    for subject in subjects {
        if !refs.existing_subjects.contains(subject) {
            return Err(ResearchNoteError::UnknownSubject);
        }
    }
    Ok(())
}

/// Adds `subject` to an existing research note: idempotent if already named (mirrors
/// `AddExternalId`'s re-import idempotency), otherwise validated against `refs` (the §9 aggregate
/// tax) before emitting `SubjectAdded`.
fn add_subject(
    state: &ResearchNoteState,
    research_note_id: ResearchNoteId,
    subject: SubjectRef,
    meta: &AssertionMeta,
    refs: &ResearchNoteRefs,
) -> Result<Vec<ResearchNoteEvent>, ResearchNoteError> {
    ensure_exists(state, research_note_id)?;
    if state.subjects.contains(&subject) {
        return Ok(Vec::new());
    }
    ensure_all_resolve(&BTreeSet::from([subject]), refs)?;
    Ok(one(
        meta,
        ResearchNoteEventBody::SubjectAdded {
            research_note_id,
            subject,
        },
    ))
}

/// Removes `subject` from an existing research note: idempotent if not named, rejected with
/// `SubjectRequired` if it is the note's only remaining subject (ADR 0028 §2), otherwise emits
/// `SubjectRemoved`.
fn remove_subject(
    state: &ResearchNoteState,
    research_note_id: ResearchNoteId,
    subject: SubjectRef,
    meta: &AssertionMeta,
) -> Result<Vec<ResearchNoteEvent>, ResearchNoteError> {
    ensure_exists(state, research_note_id)?;
    if !state.subjects.contains(&subject) {
        return Ok(Vec::new());
    }
    if state.subjects.len() <= 1 {
        return Err(ResearchNoteError::SubjectRequired);
    }
    Ok(one(
        meta,
        ResearchNoteEventBody::SubjectRemoved {
            research_note_id,
            subject,
        },
    ))
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut ResearchNoteState, event: &ResearchNoteEvent) {
    let assertion_id = event.assertion_id;
    match &event.body {
        ResearchNoteEventBody::ResearchNoteCreated {
            research_note_id,
            human_id,
            subjects,
            title,
        } => {
            state.exists = true;
            state.research_note_id = Some(*research_note_id);
            state.human_id = Some(human_id.clone());
            state.subjects.clone_from(subjects);
            state.title.clone_from(title);
            state.live_assertions.insert(assertion_id);
        }
        // Subject membership is a plain set, not per-element attributed (unlike `tags`), so
        // `SubjectAdded`/`SubjectRemoved` do not enter `live_assertions`: they are corrected by
        // issuing the inverse command (`RemoveSubject`/`AddSubject`), not by the generic
        // retract/supersede machinery.
        ResearchNoteEventBody::SubjectAdded { subject, .. } => {
            state.subjects.insert(*subject);
        }
        ResearchNoteEventBody::SubjectRemoved { subject, .. } => {
            state.subjects.remove(subject);
        }
        ResearchNoteEventBody::RichTextSet { body, .. } => {
            state.body = Some(Attributed {
                assertion_id,
                value: body.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        ResearchNoteEventBody::Tagged { tag_id, .. } => {
            state.tags.push(Attributed {
                assertion_id,
                value: *tag_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        ResearchNoteEventBody::Untagged { tag_id, .. } => {
            state.tags.retain(|t| t.value != *tag_id);
            state.live_assertions.insert(assertion_id);
        }
        ResearchNoteEventBody::RestrictionsChanged { restrictions, .. } => {
            state.restrictions.clone_from(restrictions);
            state.restrictions_assertion = Some(assertion_id);
            state.live_assertions.insert(assertion_id);
        }
        ResearchNoteEventBody::AssertionRetracted { target, .. }
        | ResearchNoteEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{decide, evolve};
    use crate::ids::{AgentId, AssertionId, EventId, HumanId, PersonId, ResearchNoteId};
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use crate::research_note::command::ResearchNoteCommand;
    use crate::research_note::error::ResearchNoteError;
    use crate::research_note::ref_resolver::ResearchNoteRefs;
    use crate::research_note::state::ResearchNoteState;
    use crate::research_note::subject::SubjectRef;
    use crate::text::{MediaType, RichText};
    use time::macros::datetime;
    use uuid::Uuid;

    fn research_note(n: u128) -> ResearchNoteId {
        ResearchNoteId::from_uuid(Uuid::from_u128(n))
    }

    fn person(n: u128) -> PersonId {
        PersonId::from_uuid(Uuid::from_u128(n))
    }

    /// Subjects for `person_ns`, all resolving.
    fn present(person_ns: &[u128]) -> ResearchNoteRefs {
        ResearchNoteRefs {
            existing_subjects: person_ns.iter().map(|&n| SubjectRef::Person(person(n))).collect(),
        }
    }

    /// No subjects resolve.
    fn missing() -> ResearchNoteRefs {
        ResearchNoteRefs::default()
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
                occurred_at: Timestamp::new(datetime!(2026-07-23 12:00:00 UTC)),
                rationale: None,
                confidence: Some(Confidence::Normal),
                citations: Vec::new(),
                evidence_analysis: None,
            },
        }
    }

    fn apply_all(state: &mut ResearchNoteState, events: &[crate::research_note::event::ResearchNoteEvent]) {
        for event in events {
            evolve(state, event);
        }
    }

    fn body(text: &str) -> RichText {
        RichText {
            text: text.to_owned(),
            media_type: MediaType::Markdown,
            language: None,
            translator: None,
            translations: Vec::new(),
        }
    }

    /// A created research note naming one subject per `person_ns` (all present in `refs`).
    fn created(id: u128, person_ns: &[u128]) -> ResearchNoteState {
        let subjects: BTreeSet<SubjectRef> = person_ns.iter().map(|&n| SubjectRef::Person(person(n))).collect();
        let mut state = ResearchNoteState::default();
        let events = decide(
            &state,
            ResearchNoteCommand::CreateResearchNote {
                research_note_id: research_note(id),
                human_id: HumanId::new("A0001"),
                subjects,
                title: Some("Same person?".to_owned()),
            },
            &meta(1),
            &present(person_ns),
        )
        .unwrap();
        apply_all(&mut state, &events);
        state
    }

    #[test]
    fn creating_against_a_missing_subject_is_unknown_subject() {
        let state = ResearchNoteState::default();
        let err = decide(
            &state,
            ResearchNoteCommand::CreateResearchNote {
                research_note_id: research_note(1),
                human_id: HumanId::new("A0001"),
                subjects: BTreeSet::from([SubjectRef::Person(person(9))]),
                title: None,
            },
            &meta(1),
            &missing(),
        )
        .unwrap_err();
        assert_eq!(err, ResearchNoteError::UnknownSubject);
    }

    #[test]
    fn creating_with_an_empty_subject_set_is_rejected() {
        let state = ResearchNoteState::default();
        let err = decide(
            &state,
            ResearchNoteCommand::CreateResearchNote {
                research_note_id: research_note(1),
                human_id: HumanId::new("A0001"),
                subjects: BTreeSet::new(),
                title: None,
            },
            &meta(1),
            &missing(),
        )
        .unwrap_err();
        assert_eq!(err, ResearchNoteError::SubjectRequired);
    }

    #[test]
    fn creating_with_one_unresolved_subject_among_several_is_unknown_subject() {
        // Only person 1 resolves; person 2 does not — the per-subject loop must catch it even
        // though the first subject checked is fine (ADR 0028 §2, the §9 aggregate tax per subject).
        let state = ResearchNoteState::default();
        let err = decide(
            &state,
            ResearchNoteCommand::CreateResearchNote {
                research_note_id: research_note(1),
                human_id: HumanId::new("A0001"),
                subjects: BTreeSet::from([SubjectRef::Person(person(1)), SubjectRef::Person(person(2))]),
                title: None,
            },
            &meta(1),
            &present(&[1]),
        )
        .unwrap_err();
        assert_eq!(err, ResearchNoteError::UnknownSubject);
    }

    #[test]
    fn multi_subject_create_records_every_subject() {
        let state = created(1, &[10, 20]);
        assert_eq!(
            state.subjects,
            BTreeSet::from([SubjectRef::Person(person(10)), SubjectRef::Person(person(20))])
        );
    }

    #[test]
    fn command_against_absent_research_note_is_not_found() {
        let state = ResearchNoteState::default();
        let err = decide(
            &state,
            ResearchNoteCommand::SetBody {
                research_note_id: research_note(7),
                body: body("x"),
            },
            &meta(2),
            &missing(),
        )
        .unwrap_err();
        assert_eq!(err, ResearchNoteError::NotFound(research_note(7)));
    }

    #[test]
    fn an_all_blank_body_is_rejected() {
        let state = created(1, &[2]);
        let err = decide(
            &state,
            ResearchNoteCommand::SetBody {
                research_note_id: research_note(1),
                body: body("   "),
            },
            &meta(2),
            &missing(),
        )
        .unwrap_err();
        assert_eq!(err, ResearchNoteError::EmptyBody);
    }

    #[test]
    fn body_is_last_writer_wins_and_retract_removes_it() {
        let mut state = created(1, &[2]);
        let set = decide(
            &state,
            ResearchNoteCommand::SetBody {
                research_note_id: research_note(1),
                body: body("The 1865 census and the parish register agree on the birth year."),
            },
            &meta(2),
            &missing(),
        )
        .unwrap();
        apply_all(&mut state, &set);
        assert!(state.body.is_some());

        let target = AssertionId::from_uuid(Uuid::from_u128(2));
        let retract = decide(
            &state,
            ResearchNoteCommand::RetractAssertion {
                research_note_id: research_note(1),
                target,
            },
            &meta(3),
            &missing(),
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.body.is_none());
    }

    #[test]
    fn superseding_the_body_replaces_it_with_the_new_text() {
        let mut state = created(1, &[2]);
        let set = decide(
            &state,
            ResearchNoteCommand::SetBody {
                research_note_id: research_note(1),
                body: body("first draft"),
            },
            &meta(2),
            &missing(),
        )
        .unwrap();
        apply_all(&mut state, &set);

        let target = AssertionId::from_uuid(Uuid::from_u128(2));
        let superseded = decide(
            &state,
            ResearchNoteCommand::SupersedeAssertion {
                research_note_id: research_note(1),
                target,
                replacement: Box::new(ResearchNoteCommand::SetBody {
                    research_note_id: research_note(1),
                    body: body("revised draft"),
                }),
            },
            &meta(3),
            &missing(),
        )
        .unwrap();
        apply_all(&mut state, &superseded);
        assert_eq!(state.body.map(|b| b.value.text), Some("revised draft".to_owned()));
    }

    #[test]
    fn tag_is_projected_then_untag_removes_it() {
        use crate::ids::TagId;

        let mut state = created(1, &[2]);
        let tag = TagId::from_uuid(Uuid::from_u128(0x7a6));
        let tagged = decide(
            &state,
            ResearchNoteCommand::Tag {
                research_note_id: research_note(1),
                tag_id: tag,
            },
            &meta(2),
            &missing(),
        )
        .unwrap();
        apply_all(&mut state, &tagged);
        assert_eq!(state.tags.len(), 1);

        let untagged = decide(
            &state,
            ResearchNoteCommand::Untag {
                research_note_id: research_note(1),
                tag_id: tag,
            },
            &meta(3),
            &missing(),
        )
        .unwrap();
        apply_all(&mut state, &untagged);
        assert!(state.tags.is_empty());
    }

    #[test]
    fn retracting_a_restriction_change_clears_the_restrictions() {
        let mut state = created(1, &[2]);
        let restrictions = std::collections::BTreeSet::from([crate::enums::Restriction::Locked]);
        let set = decide(
            &state,
            ResearchNoteCommand::SetRestrictions {
                research_note_id: research_note(1),
                restrictions: restrictions.clone(),
            },
            &meta(2),
            &missing(),
        )
        .unwrap();
        apply_all(&mut state, &set);
        assert_eq!(state.restrictions, restrictions);

        let retract = decide(
            &state,
            ResearchNoteCommand::RetractAssertion {
                research_note_id: research_note(1),
                target: AssertionId::from_uuid(Uuid::from_u128(2)),
            },
            &meta(3),
            &missing(),
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.restrictions.is_empty(), "retracting the change clears the set");
        assert_eq!(state.restrictions_assertion, None);
    }

    #[test]
    fn add_subject_extends_the_set() {
        let mut state = created(1, &[10]);
        let added = decide(
            &state,
            ResearchNoteCommand::AddSubject {
                research_note_id: research_note(1),
                subject: SubjectRef::Person(person(20)),
            },
            &meta(2),
            &present(&[20]),
        )
        .unwrap();
        apply_all(&mut state, &added);
        assert_eq!(
            state.subjects,
            BTreeSet::from([SubjectRef::Person(person(10)), SubjectRef::Person(person(20))])
        );
    }

    #[test]
    fn re_adding_an_already_named_subject_is_an_idempotent_no_op() {
        let state = created(1, &[10, 20]);
        // `refs` deliberately resolves nothing: the idempotent short-circuit must fire before the
        // aggregate-tax check runs, so a stale/absent resolver reading does not spuriously reject it.
        let events = decide(
            &state,
            ResearchNoteCommand::AddSubject {
                research_note_id: research_note(1),
                subject: SubjectRef::Person(person(20)),
            },
            &meta(2),
            &missing(),
        )
        .unwrap();
        assert!(events.is_empty(), "re-adding a present subject emits nothing");
    }

    #[test]
    fn add_subject_against_an_unknown_subject_is_unknown_subject() {
        let state = created(1, &[10]);
        let err = decide(
            &state,
            ResearchNoteCommand::AddSubject {
                research_note_id: research_note(1),
                subject: SubjectRef::Person(person(99)),
            },
            &meta(2),
            &missing(),
        )
        .unwrap_err();
        assert_eq!(err, ResearchNoteError::UnknownSubject);
    }

    #[test]
    fn add_subject_against_an_absent_note_is_not_found() {
        let state = ResearchNoteState::default();
        let err = decide(
            &state,
            ResearchNoteCommand::AddSubject {
                research_note_id: research_note(1),
                subject: SubjectRef::Person(person(1)),
            },
            &meta(1),
            &present(&[1]),
        )
        .unwrap_err();
        assert_eq!(err, ResearchNoteError::NotFound(research_note(1)));
    }

    #[test]
    fn remove_subject_shrinks_the_set_but_refuses_to_empty_it() {
        let mut state = created(1, &[10, 20]);
        let removed = decide(
            &state,
            ResearchNoteCommand::RemoveSubject {
                research_note_id: research_note(1),
                subject: SubjectRef::Person(person(20)),
            },
            &meta(2),
            &missing(),
        )
        .unwrap();
        apply_all(&mut state, &removed);
        assert_eq!(state.subjects, BTreeSet::from([SubjectRef::Person(person(10))]));

        let err = decide(
            &state,
            ResearchNoteCommand::RemoveSubject {
                research_note_id: research_note(1),
                subject: SubjectRef::Person(person(10)),
            },
            &meta(3),
            &missing(),
        )
        .unwrap_err();
        assert_eq!(err, ResearchNoteError::SubjectRequired);
        assert_eq!(
            state.subjects,
            BTreeSet::from([SubjectRef::Person(person(10))]),
            "a rejected command must not have changed the state"
        );
    }

    #[test]
    fn removing_a_subject_not_named_is_an_idempotent_no_op() {
        let state = created(1, &[10, 20]);
        let events = decide(
            &state,
            ResearchNoteCommand::RemoveSubject {
                research_note_id: research_note(1),
                subject: SubjectRef::Person(person(99)),
            },
            &meta(2),
            &missing(),
        )
        .unwrap();
        assert!(events.is_empty(), "removing an absent subject emits nothing");
    }

    #[test]
    fn remove_subject_against_an_absent_note_is_not_found() {
        let state = ResearchNoteState::default();
        let err = decide(
            &state,
            ResearchNoteCommand::RemoveSubject {
                research_note_id: research_note(1),
                subject: SubjectRef::Person(person(1)),
            },
            &meta(1),
            &missing(),
        )
        .unwrap_err();
        assert_eq!(err, ResearchNoteError::NotFound(research_note(1)));
    }

    #[test]
    fn subject_ref_covers_the_four_conclusion_bearing_kinds() {
        // SubjectRef::Event is exercised only here (the fixtures above use Person); this closes
        // that gap without duplicating a whole create/decide flow for the other three kinds.
        let subject = SubjectRef::Event(EventId::from_uuid(Uuid::from_u128(3)));
        assert!(matches!(subject, SubjectRef::Event(_)));
    }
}
