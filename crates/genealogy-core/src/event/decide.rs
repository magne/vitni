//! The pure Event decision core (ADR 0004 §3) and the `evolve` fold.
//!
//! `decide(state, command, meta, refs)` reads no clock, generates no id, and reads no other
//! aggregate's projection itself: the cross-aggregate facts arrive in `refs`, resolved by the
//! `Services`-backed adapter from the [`EventRefResolver`](super::ref_resolver). So the rule
//! (`UnknownPlace`) lives here, in the pure core, while the impure read stays at the edge.

use crate::assertions::Attributed;
use crate::event::command::EventCommand;
use crate::event::error::EventError;
use crate::event::events::{EventEvent, EventEventBody};
use crate::event::ref_resolver::EventRefs;
use crate::event::state::EventState;
use crate::ids::EventId;
use crate::provenance::AssertionMeta;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns an [`EventError`] when the command violates an invariant: creating an event that
/// exists, a command against an absent event, linking a place the projection does not know
/// (`refs.place_exists == false`, the §9 aggregate-tax check), or correcting an unknown assertion.
pub fn decide(
    state: &EventState,
    command: EventCommand,
    meta: &AssertionMeta,
    refs: &EventRefs,
) -> Result<Vec<EventEvent>, EventError> {
    match command {
        EventCommand::CreateEvent {
            event_id,
            human_id,
            event_type,
            private,
        } => {
            if state.exists {
                return Err(EventError::AlreadyExists(event_id));
            }
            Ok(one(
                meta,
                EventEventBody::EventCreated {
                    event_id,
                    human_id,
                    event_type,
                    private,
                },
            ))
        }
        // The single-fact setters (type/date/description/participant/citation/media/note/tag) all
        // share the same shape — exist-check then emit one event — so they delegate to `setter_body`
        // (which is exhaustive over them). Only `event_id` is bound here (it is `Copy`), leaving
        // `command` intact to hand over.
        EventCommand::SetEventType { event_id, .. }
        | EventCommand::AssertDate { event_id, .. }
        | EventCommand::SetDescription { event_id, .. }
        | EventCommand::AddParticipantRole { event_id, .. }
        | EventCommand::RemoveParticipantRole { event_id, .. }
        | EventCommand::AddCitation { event_id, .. }
        | EventCommand::AttachMedia { event_id, .. }
        | EventCommand::AttachNote { event_id, .. }
        | EventCommand::Tag { event_id, .. }
        | EventCommand::Untag { event_id, .. } => {
            ensure_exists(state, event_id)?;
            Ok(one(meta, setter_body(command)))
        }
        EventCommand::LinkPlace { event_id, place_id } => {
            ensure_exists(state, event_id)?;
            if !refs.place_exists {
                return Err(EventError::UnknownPlace(place_id));
            }
            Ok(one(meta, EventEventBody::PlaceLinked { event_id, place_id }))
        }
        EventCommand::RetractAssertion { event_id, target } => {
            ensure_exists(state, event_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(EventError::RetractsMissingAssertion(target));
            }
            Ok(one(meta, EventEventBody::AssertionRetracted { event_id, target }))
        }
        EventCommand::SupersedeAssertion {
            event_id,
            target,
            replacement,
        } => {
            ensure_exists(state, event_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(EventError::SupersedesMissingAssertion(target));
            }
            let mut events = one(meta, EventEventBody::AssertionSuperseded { event_id, target });
            events.extend(decide(state, *replacement, meta, refs)?);
            Ok(events)
        }
    }
}

/// Maps a single-fact setter command to its event body (the existence check is done by `decide`).
///
/// Exhaustive over the setter commands; the lifecycle/cross-aggregate commands never reach here.
fn setter_body(command: EventCommand) -> EventEventBody {
    match command {
        EventCommand::SetEventType { event_id, event_type } => EventEventBody::EventTypeSet { event_id, event_type },
        EventCommand::AssertDate { event_id, date } => EventEventBody::DateAsserted { event_id, date },
        EventCommand::SetDescription { event_id, description } => {
            EventEventBody::DescriptionSet { event_id, description }
        }
        EventCommand::AddParticipantRole {
            event_id,
            participant_id,
            role,
        } => EventEventBody::ParticipantRoleAdded {
            event_id,
            participant_id,
            role,
        },
        EventCommand::RemoveParticipantRole {
            event_id,
            participant_id,
            role,
        } => EventEventBody::ParticipantRoleRemoved {
            event_id,
            participant_id,
            role,
        },
        EventCommand::AddCitation { event_id, citation_id } => EventEventBody::CitationAdded { event_id, citation_id },
        EventCommand::AttachMedia { event_id, media } => EventEventBody::MediaAttached { event_id, media },
        EventCommand::AttachNote { event_id, note_id } => EventEventBody::NoteAttached { event_id, note_id },
        EventCommand::Tag { event_id, tag_id } => EventEventBody::Tagged { event_id, tag_id },
        EventCommand::Untag { event_id, tag_id } => EventEventBody::Untagged { event_id, tag_id },
        EventCommand::CreateEvent { .. }
        | EventCommand::LinkPlace { .. }
        | EventCommand::RetractAssertion { .. }
        | EventCommand::SupersedeAssertion { .. } => unreachable!("handled by decide"),
    }
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: EventEventBody) -> Vec<EventEvent> {
    vec![EventEvent::new(meta, body)]
}

/// Rejects a command that targets an event which has not been created yet.
fn ensure_exists(state: &EventState, event_id: EventId) -> Result<(), EventError> {
    if state.exists {
        Ok(())
    } else {
        Err(EventError::NotFound(event_id))
    }
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut EventState, event: &EventEvent) {
    let assertion_id = event.assertion_id;
    match &event.body {
        EventEventBody::EventCreated {
            event_id,
            human_id,
            event_type,
            private,
        } => {
            state.exists = true;
            state.event_id = Some(*event_id);
            state.human_id = Some(human_id.clone());
            state.event_type = Some(Attributed {
                assertion_id,
                value: event_type.clone(),
            });
            state.private = *private;
            state.live_assertions.insert(assertion_id);
        }
        EventEventBody::EventTypeSet { event_type, .. } => {
            state.event_type = Some(Attributed {
                assertion_id,
                value: event_type.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        EventEventBody::DateAsserted { date, .. } => {
            state.date = Some(Attributed {
                assertion_id,
                value: date.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        EventEventBody::DescriptionSet { description, .. } => {
            state.description = Some(Attributed {
                assertion_id,
                value: description.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        EventEventBody::PlaceLinked { place_id, .. } => {
            state.place_id = Some(Attributed {
                assertion_id,
                value: *place_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        EventEventBody::ParticipantRoleAdded {
            participant_id, role, ..
        } => {
            state.participants.push(Attributed {
                assertion_id,
                value: crate::event::state::EventParticipant {
                    participant_id: *participant_id,
                    role: role.clone(),
                },
            });
            state.live_assertions.insert(assertion_id);
        }
        EventEventBody::ParticipantRoleRemoved {
            participant_id, role, ..
        } => {
            state
                .participants
                .retain(|p| !(p.value.participant_id == *participant_id && p.value.role == *role));
            state.live_assertions.insert(assertion_id);
        }
        EventEventBody::CitationAdded { .. }
        | EventEventBody::MediaAttached { .. }
        | EventEventBody::NoteAttached { .. }
        | EventEventBody::Tagged { .. }
        | EventEventBody::Untagged { .. } => {
            state.live_assertions.insert(assertion_id);
        }
        EventEventBody::AssertionRetracted { target, .. } | EventEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::assertions::EventBody;
    use crate::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
    use crate::enums::EventType;
    use crate::event::command::EventCommand;
    use crate::event::error::EventError;
    use crate::event::events::EventEventBody;
    use crate::event::ref_resolver::EventRefs;
    use crate::event::state::EventState;
    use crate::ids::{AgentId, AssertionId, EventId, HumanId, PlaceId};
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use time::macros::datetime;
    use uuid::Uuid;

    fn event(n: u128) -> EventId {
        EventId::from_uuid(Uuid::from_u128(n))
    }

    fn place(n: u128) -> PlaceId {
        PlaceId::from_uuid(Uuid::from_u128(n))
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

    const PLACE_PRESENT: EventRefs = EventRefs { place_exists: true };
    const PLACE_MISSING: EventRefs = EventRefs { place_exists: false };

    fn a_date() -> GenealogicalDate {
        GenealogicalDate {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Normal,
            modifier: GenealogicalDateBody::Structured(DateModifier::None(DatePoint {
                year: Some(1847),
                month: Some(3),
                day: Some(12),
            })),
            new_year_begins: None,
            sort_value: 18_470_312,
            original_text: None,
        }
    }

    fn apply_all(state: &mut EventState, events: &[crate::event::events::EventEvent]) {
        for event in events {
            evolve(state, event);
        }
    }

    fn created_event(id: u128) -> EventState {
        let mut state = EventState::default();
        let events = decide(
            &state,
            EventCommand::CreateEvent {
                event_id: event(id),
                human_id: HumanId::new("E1"),
                event_type: EventType::Birth,
                private: false,
            },
            &meta(1),
            &PLACE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &events);
        state
    }

    #[test]
    fn create_event_on_empty_state_emits_event_created() {
        let state = EventState::default();
        let events = decide(
            &state,
            EventCommand::CreateEvent {
                event_id: event(1),
                human_id: HumanId::new("E1"),
                event_type: EventType::Marriage,
                private: false,
            },
            &meta(1),
            &PLACE_PRESENT,
        )
        .unwrap();
        assert!(matches!(events[0].body, EventEventBody::EventCreated { .. }));
    }

    #[test]
    fn create_event_records_the_private_flag() {
        let mut state = EventState::default();
        let events = decide(
            &state,
            EventCommand::CreateEvent {
                event_id: event(1),
                human_id: HumanId::new("E1"),
                event_type: EventType::Birth,
                private: true,
            },
            &meta(1),
            &PLACE_PRESENT,
        )
        .unwrap();
        assert_eq!(events[0].body.version(), "2.0");
        apply_all(&mut state, &events);
        assert!(state.private);
    }

    #[test]
    fn linking_a_present_place_emits_place_linked() {
        let state = created_event(1);
        let events = decide(
            &state,
            EventCommand::LinkPlace {
                event_id: event(1),
                place_id: place(1),
            },
            &meta(2),
            &PLACE_PRESENT,
        )
        .unwrap();
        assert!(matches!(events[0].body, EventEventBody::PlaceLinked { .. }));
    }

    #[test]
    fn linking_a_missing_place_is_unknown_place() {
        // The aggregate-tax check: the resolver reported the place absent, so `decide` rejects
        // with the domain error (proving the Services path, not an app-layer guard).
        let state = created_event(1);
        let err = decide(
            &state,
            EventCommand::LinkPlace {
                event_id: event(1),
                place_id: place(99),
            },
            &meta(2),
            &PLACE_MISSING,
        )
        .unwrap_err();
        assert_eq!(err, EventError::UnknownPlace(place(99)));
    }

    #[test]
    fn command_against_absent_event_is_not_found() {
        let state = EventState::default();
        let err = decide(
            &state,
            EventCommand::AssertDate {
                event_id: event(7),
                date: a_date(),
            },
            &meta(2),
            &PLACE_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, EventError::NotFound(event(7)));
    }

    #[test]
    fn date_and_type_are_recorded_last_writer_wins() {
        let mut state = created_event(1);
        for events in [
            decide(
                &state,
                EventCommand::AssertDate {
                    event_id: event(1),
                    date: a_date(),
                },
                &meta(2),
                &PLACE_PRESENT,
            )
            .unwrap(),
            decide(
                &state,
                EventCommand::SetEventType {
                    event_id: event(1),
                    event_type: EventType::Baptism,
                },
                &meta(3),
                &PLACE_PRESENT,
            )
            .unwrap(),
        ] {
            apply_all(&mut state, &events);
        }
        assert_eq!(state.event_type.as_ref().map(|t| &t.value), Some(&EventType::Baptism));
        assert_eq!(state.date.as_ref().map(|d| d.value.sort_value), Some(18_470_312));
    }

    #[test]
    fn retracting_an_unknown_assertion_is_rejected() {
        let state = created_event(1);
        let unknown = AssertionId::from_uuid(Uuid::from_u128(999));
        let err = decide(
            &state,
            EventCommand::RetractAssertion {
                event_id: event(1),
                target: unknown,
            },
            &meta(2),
            &PLACE_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, EventError::RetractsMissingAssertion(unknown));
    }

    #[test]
    fn retracting_a_live_date_removes_it_non_destructively() {
        let mut state = created_event(1);
        let date_events = decide(
            &state,
            EventCommand::AssertDate {
                event_id: event(1),
                date: a_date(),
            },
            &meta(2),
            &PLACE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &date_events);
        let date_assertion = AssertionId::from_uuid(Uuid::from_u128(2));
        assert!(state.date.is_some());

        let retract = decide(
            &state,
            EventCommand::RetractAssertion {
                event_id: event(1),
                target: date_assertion,
            },
            &meta(3),
            &PLACE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &retract);

        assert!(state.date.is_none());
        assert!(!state.live_assertions.contains(&date_assertion));
    }

    #[test]
    fn superseding_a_linked_place_emits_supersession_then_replacement() {
        let mut state = created_event(1);
        let first = decide(
            &state,
            EventCommand::LinkPlace {
                event_id: event(1),
                place_id: place(1),
            },
            &meta(2),
            &PLACE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &first);
        let target = AssertionId::from_uuid(Uuid::from_u128(2));

        let events = decide(
            &state,
            EventCommand::SupersedeAssertion {
                event_id: event(1),
                target,
                replacement: Box::new(EventCommand::LinkPlace {
                    event_id: event(1),
                    place_id: place(2),
                }),
            },
            &meta(3),
            &PLACE_PRESENT,
        )
        .unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].body, EventEventBody::AssertionSuperseded { .. }));
        assert!(matches!(events[1].body, EventEventBody::PlaceLinked { .. }));

        apply_all(&mut state, &events);
        assert_eq!(state.place_id.as_ref().map(|p| p.value), Some(place(2)));
        assert!(!state.live_assertions.contains(&target));
    }

    #[test]
    fn participants_accumulate_and_remove_drops_the_matching_one() {
        use crate::enums::ParticipantRole;
        use crate::ids::PersonId;

        let person = PersonId::from_uuid(Uuid::from_u128(0x50));
        let mut state = created_event(1);
        let add = decide(
            &state,
            EventCommand::AddParticipantRole {
                event_id: event(1),
                participant_id: person,
                role: ParticipantRole::Primary,
            },
            &meta(2),
            &PLACE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &add);
        assert_eq!(state.participants.len(), 1);

        let remove = decide(
            &state,
            EventCommand::RemoveParticipantRole {
                event_id: event(1),
                participant_id: person,
                role: ParticipantRole::Primary,
            },
            &meta(3),
            &PLACE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &remove);
        assert!(state.participants.is_empty());
    }
}
