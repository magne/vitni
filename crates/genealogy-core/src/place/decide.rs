//! The pure Place decision core (ADR 0004 §3) and the `evolve` fold.
//!
//! `decide(state, command, meta) -> Result<Vec<PlaceEvent>, PlaceError>` reads no clock and
//! generates no id (those arrive in `meta`), so it is unit-testable with no I/O. `evolve` applies
//! an event to the state.

use crate::ids::PlaceId;
use crate::place::command::PlaceCommand;
use crate::place::error::PlaceError;
use crate::place::event::{PlaceEvent, PlaceEventBody};
use crate::place::state::PlaceState;
use crate::provenance::AssertionMeta;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`PlaceError`] when the command violates a within-aggregate invariant: creating a
/// place that exists, a command against an absent place, or an empty name.
pub fn decide(state: &PlaceState, command: PlaceCommand, meta: &AssertionMeta) -> Result<Vec<PlaceEvent>, PlaceError> {
    let body = match command {
        PlaceCommand::CreatePlace {
            place_id,
            human_id,
            place_type,
        } => {
            if state.exists {
                return Err(PlaceError::AlreadyExists(place_id));
            }
            PlaceEventBody::PlaceCreated {
                place_id,
                human_id,
                place_type,
            }
        }
        PlaceCommand::SetPlaceType { place_id, place_type } => {
            ensure_exists(state, place_id)?;
            PlaceEventBody::PlaceTypeSet { place_id, place_type }
        }
        PlaceCommand::AssertName { place_id, name } => {
            ensure_exists(state, place_id)?;
            if name.is_empty() {
                return Err(PlaceError::EmptyName);
            }
            PlaceEventBody::NameAsserted { place_id, name }
        }
    };
    Ok(vec![PlaceEvent::new(meta, body)])
}

/// Rejects a command that targets a place which has not been created yet.
fn ensure_exists(state: &PlaceState, place_id: PlaceId) -> Result<(), PlaceError> {
    if state.exists {
        Ok(())
    } else {
        Err(PlaceError::NotFound(place_id))
    }
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut PlaceState, event: &PlaceEvent) {
    match &event.body {
        PlaceEventBody::PlaceCreated {
            place_id,
            human_id,
            place_type,
        } => {
            state.exists = true;
            state.place_id = Some(*place_id);
            state.human_id = Some(human_id.clone());
            state.place_type = Some(place_type.clone());
        }
        PlaceEventBody::PlaceTypeSet { place_type, .. } => {
            state.place_type = Some(place_type.clone());
        }
        PlaceEventBody::NameAsserted { name, .. } => {
            state.names.push(name.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::enums::PlaceType;
    use crate::ids::{AgentId, AssertionId, HumanId, PlaceId};
    use crate::place::command::PlaceCommand;
    use crate::place::error::PlaceError;
    use crate::place::event::PlaceEventBody;
    use crate::place::state::PlaceState;
    use crate::place_name::PlaceName;
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use time::macros::datetime;
    use uuid::Uuid;

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

    fn named(text: &str) -> PlaceName {
        PlaceName {
            text: text.to_owned(),
            language: None,
            date: None,
        }
    }

    fn apply_all(state: &mut PlaceState, events: &[crate::place::event::PlaceEvent]) {
        for event in events {
            evolve(state, event);
        }
    }

    fn created_place(id: u128) -> PlaceState {
        let mut state = PlaceState::default();
        let events = decide(
            &state,
            PlaceCommand::CreatePlace {
                place_id: place(id),
                human_id: HumanId::new("P1"),
                place_type: PlaceType::Parish,
            },
            &meta(1),
        )
        .unwrap();
        apply_all(&mut state, &events);
        state
    }

    #[test]
    fn create_place_on_empty_state_emits_place_created() {
        let state = PlaceState::default();
        let events = decide(
            &state,
            PlaceCommand::CreatePlace {
                place_id: place(1),
                human_id: HumanId::new("P1"),
                place_type: PlaceType::Farm,
            },
            &meta(1),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].body, PlaceEventBody::PlaceCreated { .. }));
    }

    #[test]
    fn recreating_an_existing_place_is_rejected() {
        let state = created_place(1);
        let err = decide(
            &state,
            PlaceCommand::CreatePlace {
                place_id: place(1),
                human_id: HumanId::new("P1"),
                place_type: PlaceType::Farm,
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::AlreadyExists(place(1)));
    }

    #[test]
    fn command_against_absent_place_is_not_found() {
        let state = PlaceState::default();
        let err = decide(
            &state,
            PlaceCommand::AssertName {
                place_id: place(7),
                name: named("Vågå"),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::NotFound(place(7)));
    }

    #[test]
    fn asserting_an_empty_name_is_rejected() {
        let state = created_place(1);
        let err = decide(
            &state,
            PlaceCommand::AssertName {
                place_id: place(1),
                name: named("  "),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::EmptyName);
    }

    #[test]
    fn asserting_names_accumulates_them_and_setting_the_type_replaces_it() {
        let mut state = created_place(1);
        for (assertion, text) in [(2, "Vågå"), (3, "Waage")] {
            let events = decide(
                &state,
                PlaceCommand::AssertName {
                    place_id: place(1),
                    name: named(text),
                },
                &meta(assertion),
            )
            .unwrap();
            apply_all(&mut state, &events);
        }
        assert_eq!(state.names.len(), 2);

        let events = decide(
            &state,
            PlaceCommand::SetPlaceType {
                place_id: place(1),
                place_type: PlaceType::Municipality,
            },
            &meta(4),
        )
        .unwrap();
        apply_all(&mut state, &events);
        assert_eq!(state.place_type, Some(PlaceType::Municipality));
    }
}
