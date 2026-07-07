//! The pure Place decision core (ADR 0004 §3) and the `evolve` fold.
//!
//! `decide(state, command, meta, refs)` reads no clock, generates no id, and reads no other
//! aggregate's projection itself: the cross-aggregate fact (does the enclosing place exist?) arrives
//! in `refs`, resolved before `decide` by the `Services`-backed adapter from the
//! [`PlaceRefResolver`](super::ref_resolver). So the rule (`UnknownPlace`) lives here, in the pure
//! core, while the impure read stays at the edge.

use crate::assertions::{Asserted, Attributed};
use crate::ids::{HumanId, PlaceId};
use crate::place::command::PlaceCommand;
use crate::place::error::PlaceError;
use crate::place::event::{PlaceEvent, PlaceEventBody};
use crate::place::ref_resolver::PlaceRefs;
use crate::place::state::PlaceState;
use crate::provenance::AssertionMeta;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`PlaceError`] when the command violates an invariant: creating a place that exists, a
/// command against an absent place, an empty name or code, enclosing the place in one the projection
/// does not know (`refs.enclosing_exists == false`, the §9 aggregate-tax check), or correcting an
/// unknown assertion.
pub fn decide(
    state: &PlaceState,
    command: PlaceCommand,
    meta: &AssertionMeta,
    refs: &PlaceRefs,
) -> Result<Vec<PlaceEvent>, PlaceError> {
    match command {
        PlaceCommand::CreatePlace {
            place_id,
            human_id,
            place_type,
        } => {
            if state.exists {
                return Err(PlaceError::AlreadyExists(place_id));
            }
            Ok(one(
                meta,
                PlaceEventBody::PlaceCreated {
                    place_id,
                    human_id,
                    place_type,
                },
            ))
        }
        PlaceCommand::SetPlaceType { place_id, place_type } => {
            ensure_exists(state, place_id)?;
            Ok(one(meta, PlaceEventBody::PlaceTypeSet { place_id, place_type }))
        }
        PlaceCommand::AssertName { place_id, name } => {
            ensure_exists(state, place_id)?;
            if name.is_empty() {
                return Err(PlaceError::EmptyName);
            }
            Ok(one(meta, PlaceEventBody::NameAsserted { place_id, name }))
        }
        PlaceCommand::AssertEnclosedBy { place_id, enclosed_by } => {
            ensure_exists(state, place_id)?;
            if !refs.enclosing_exists {
                return Err(PlaceError::UnknownPlace(enclosed_by.place_id));
            }
            Ok(one(meta, PlaceEventBody::EnclosedByAsserted { place_id, enclosed_by }))
        }
        PlaceCommand::AssertCoordinates { place_id, coordinates } => {
            ensure_exists(state, place_id)?;
            Ok(one(meta, PlaceEventBody::CoordinatesAsserted { place_id, coordinates }))
        }
        PlaceCommand::SetCode { place_id, code } => {
            ensure_exists(state, place_id)?;
            if code.trim().is_empty() {
                return Err(PlaceError::EmptyCode);
            }
            Ok(one(meta, PlaceEventBody::CodeSet { place_id, code }))
        }
        PlaceCommand::AddCitation { place_id, citation_id } => {
            ensure_exists(state, place_id)?;
            Ok(one(meta, PlaceEventBody::CitationAdded { place_id, citation_id }))
        }
        PlaceCommand::AttachMedia { place_id, media } => {
            ensure_exists(state, place_id)?;
            Ok(one(meta, PlaceEventBody::MediaAttached { place_id, media }))
        }
        PlaceCommand::AttachNote { place_id, note_id } => {
            ensure_exists(state, place_id)?;
            Ok(one(meta, PlaceEventBody::NoteAttached { place_id, note_id }))
        }
        PlaceCommand::Tag { place_id, tag_id } => {
            ensure_exists(state, place_id)?;
            Ok(one(meta, PlaceEventBody::Tagged { place_id, tag_id }))
        }
        PlaceCommand::Untag { place_id, tag_id } => {
            ensure_exists(state, place_id)?;
            Ok(one(meta, PlaceEventBody::Untagged { place_id, tag_id }))
        }
        PlaceCommand::SetRestrictions { place_id, restrictions } => {
            ensure_exists(state, place_id)?;
            Ok(one(
                meta,
                PlaceEventBody::RestrictionsChanged { place_id, restrictions },
            ))
        }
        PlaceCommand::SetHumanId { place_id, human_id } => {
            ensure_exists(state, place_id)?;
            Ok(one(meta, place_human_id_changed(state, place_id, human_id)))
        }
        PlaceCommand::RetractAssertion { place_id, target } => {
            ensure_exists(state, place_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(PlaceError::RetractsMissingAssertion(target));
            }
            Ok(one(meta, PlaceEventBody::AssertionRetracted { place_id, target }))
        }
        PlaceCommand::SupersedeAssertion {
            place_id,
            target,
            replacement,
        } => {
            ensure_exists(state, place_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(PlaceError::SupersedesMissingAssertion(target));
            }
            let mut events = one(meta, PlaceEventBody::AssertionSuperseded { place_id, target });
            events.extend(decide(state, *replacement, meta, refs)?);
            Ok(events)
        }
    }
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: PlaceEventBody) -> Vec<PlaceEvent> {
    vec![PlaceEvent::new(meta, body)]
}

/// Builds the `HumanIdChanged` body, carrying the id in effect before the change for the audit trail.
fn place_human_id_changed(state: &PlaceState, place_id: PlaceId, human_id: HumanId) -> PlaceEventBody {
    let old_human_id = state.human_id.clone().unwrap_or_else(|| human_id.clone());
    PlaceEventBody::HumanIdChanged {
        place_id,
        human_id,
        old_human_id,
    }
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
    let assertion_id = event.assertion_id;
    match &event.body {
        PlaceEventBody::PlaceCreated {
            place_id,
            human_id,
            place_type,
        } => {
            state.exists = true;
            state.place_id = Some(*place_id);
            state.human_id = Some(human_id.clone());
            state.place_type = Some(Attributed {
                assertion_id,
                value: Asserted::from_context(place_type.clone(), &event.context),
            });
            state.live_assertions.insert(assertion_id);
        }
        PlaceEventBody::PlaceTypeSet { place_type, .. } => {
            state.place_type = Some(Attributed {
                assertion_id,
                value: Asserted::from_context(place_type.clone(), &event.context),
            });
            state.live_assertions.insert(assertion_id);
        }
        PlaceEventBody::NameAsserted { name, .. } => {
            state.names.push(Attributed {
                assertion_id,
                value: Asserted::from_context(name.clone(), &event.context),
            });
            state.live_assertions.insert(assertion_id);
        }
        PlaceEventBody::EnclosedByAsserted { enclosed_by, .. } => {
            state.enclosed_by.push(Attributed {
                assertion_id,
                value: Asserted::from_context(enclosed_by.clone(), &event.context),
            });
            state.live_assertions.insert(assertion_id);
        }
        PlaceEventBody::CoordinatesAsserted { coordinates, .. } => {
            state.coordinates = Some(Attributed {
                assertion_id,
                value: Asserted::from_context(*coordinates, &event.context),
            });
            state.live_assertions.insert(assertion_id);
        }
        PlaceEventBody::CodeSet { code, .. } => {
            state.code = Some(Attributed {
                assertion_id,
                value: Asserted::from_context(code.clone(), &event.context),
            });
            state.live_assertions.insert(assertion_id);
        }
        PlaceEventBody::CitationAdded { citation_id, .. } => {
            state.citations.push(Attributed {
                assertion_id,
                value: *citation_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        PlaceEventBody::MediaAttached { media, .. } => {
            state.media.push(Attributed {
                assertion_id,
                value: media.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        PlaceEventBody::NoteAttached { note_id, .. } => {
            state.notes.push(Attributed {
                assertion_id,
                value: *note_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        PlaceEventBody::Tagged { tag_id, .. } => {
            state.tags.push(Attributed {
                assertion_id,
                value: *tag_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        PlaceEventBody::Untagged { tag_id, .. } => {
            state.tags.retain(|t| t.value != *tag_id);
            state.live_assertions.insert(assertion_id);
        }
        PlaceEventBody::RestrictionsChanged { restrictions, .. } => {
            state.restrictions.clone_from(restrictions);
            state.live_assertions.insert(assertion_id);
        }
        PlaceEventBody::HumanIdChanged { human_id, .. } => {
            state.human_id = Some(human_id.clone());
        }
        PlaceEventBody::AssertionRetracted { target, .. } | PlaceEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::enums::PlaceType;
    use crate::geo::{GeoCoordinates, Microdegrees};
    use crate::ids::{AgentId, AssertionId, HumanId, PlaceId};
    use crate::place::command::PlaceCommand;
    use crate::place::error::PlaceError;
    use crate::place::event::PlaceEventBody;
    use crate::place::ref_resolver::PlaceRefs;
    use crate::place::state::PlaceState;
    use crate::place_name::PlaceName;
    use crate::place_ref::PlaceRef;
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use time::macros::datetime;
    use uuid::Uuid;

    const ENCLOSING_PRESENT: PlaceRefs = PlaceRefs { enclosing_exists: true };
    const ENCLOSING_MISSING: PlaceRefs = PlaceRefs {
        enclosing_exists: false,
    };

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
            &ENCLOSING_PRESENT,
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
            &ENCLOSING_PRESENT,
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
            &ENCLOSING_PRESENT,
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
            &ENCLOSING_PRESENT,
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
            &ENCLOSING_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::EmptyName);
    }

    #[test]
    fn setting_an_empty_code_is_rejected() {
        let state = created_place(1);
        let err = decide(
            &state,
            PlaceCommand::SetCode {
                place_id: place(1),
                code: "  ".to_owned(),
            },
            &meta(2),
            &ENCLOSING_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::EmptyCode);
    }

    #[test]
    fn enclosing_in_a_missing_place_is_unknown_place() {
        let state = created_place(1);
        let err = decide(
            &state,
            PlaceCommand::AssertEnclosedBy {
                place_id: place(1),
                enclosed_by: PlaceRef {
                    place_id: place(99),
                    date: None,
                },
            },
            &meta(2),
            &ENCLOSING_MISSING,
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::UnknownPlace(place(99)));
    }

    #[test]
    fn enclosing_in_a_present_place_accumulates_and_coordinates_are_last_writer_wins() {
        let mut state = created_place(1);
        let enclosed = decide(
            &state,
            PlaceCommand::AssertEnclosedBy {
                place_id: place(1),
                enclosed_by: PlaceRef {
                    place_id: place(2),
                    date: None,
                },
            },
            &meta(2),
            &ENCLOSING_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &enclosed);
        assert_eq!(state.enclosed_by.len(), 1);

        let coords = decide(
            &state,
            PlaceCommand::AssertCoordinates {
                place_id: place(1),
                coordinates: GeoCoordinates {
                    latitude: Microdegrees::from_microdegrees(60_391_262),
                    longitude: Microdegrees::from_microdegrees(5_322_054),
                },
            },
            &meta(3),
            &ENCLOSING_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &coords);
        assert!(state.coordinates.is_some());
    }

    #[test]
    fn attachments_only_register_live_assertions() {
        let mut state = created_place(1);
        let tagged = decide(
            &state,
            PlaceCommand::Tag {
                place_id: place(1),
                tag_id: crate::ids::TagId::from_uuid(Uuid::from_u128(0x7)),
            },
            &meta(2),
            &ENCLOSING_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &tagged);
        assert!(
            state
                .live_assertions
                .contains(&AssertionId::from_uuid(Uuid::from_u128(2)))
        );
    }

    #[test]
    fn retracting_a_live_name_removes_it_non_destructively() {
        let mut state = created_place(1);
        let name_events = decide(
            &state,
            PlaceCommand::AssertName {
                place_id: place(1),
                name: named("Vågå"),
            },
            &meta(2),
            &ENCLOSING_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &name_events);
        let name_assertion = AssertionId::from_uuid(Uuid::from_u128(2));
        assert_eq!(state.names.len(), 1);

        let retract = decide(
            &state,
            PlaceCommand::RetractAssertion {
                place_id: place(1),
                target: name_assertion,
            },
            &meta(3),
            &ENCLOSING_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &retract);

        assert!(state.names.is_empty());
        assert!(!state.live_assertions.contains(&name_assertion));
    }

    #[test]
    fn retracting_an_unknown_assertion_is_rejected() {
        let state = created_place(1);
        let unknown = AssertionId::from_uuid(Uuid::from_u128(999));
        let err = decide(
            &state,
            PlaceCommand::RetractAssertion {
                place_id: place(1),
                target: unknown,
            },
            &meta(2),
            &ENCLOSING_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::RetractsMissingAssertion(unknown));
    }

    #[test]
    fn superseding_a_name_emits_supersession_then_replacement() {
        let mut state = created_place(1);
        let first = decide(
            &state,
            PlaceCommand::AssertName {
                place_id: place(1),
                name: named("Vågå"),
            },
            &meta(2),
            &ENCLOSING_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &first);
        let target = AssertionId::from_uuid(Uuid::from_u128(2));

        let events = decide(
            &state,
            PlaceCommand::SupersedeAssertion {
                place_id: place(1),
                target,
                replacement: Box::new(PlaceCommand::AssertName {
                    place_id: place(1),
                    name: named("Waage"),
                }),
            },
            &meta(3),
            &ENCLOSING_PRESENT,
        )
        .unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].body, PlaceEventBody::AssertionSuperseded { .. }));
        assert!(matches!(events[1].body, PlaceEventBody::NameAsserted { .. }));

        apply_all(&mut state, &events);
        assert_eq!(state.names.len(), 1);
        assert_eq!(state.names[0].value.value.text, "Waage");
        assert!(!state.live_assertions.contains(&target));
    }
}
