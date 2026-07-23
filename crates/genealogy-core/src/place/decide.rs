//! The pure Place decision core (ADR 0004 §3) and the `evolve` fold.
//!
//! `decide(state, command, meta, refs)` reads no clock, generates no id, and reads no other
//! aggregate's projection itself: the cross-aggregate fact (does the enclosing place exist?) arrives
//! in `refs`, resolved before `decide` by the `Services`-backed adapter from the
//! [`PlaceRefResolver`](super::ref_resolver). So the rule (`UnknownPlace`) lives here, in the pure
//! core, while the impure read stays at the edge.

use crate::assertions::{Asserted, Attributed};
use crate::enums::{PlaceType, SuccessionKind};
use crate::geo::PlaceGeometry;
use crate::ids::{HumanId, PlaceId};
use crate::place::command::PlaceCommand;
use crate::place::error::PlaceError;
use crate::place::event::{PlaceEvent, PlaceEventBody};
use crate::place::ref_resolver::PlaceRefs;
use crate::place::state::PlaceState;
use crate::place_geometry::PlaceGeometryAssertion;
use crate::place_succession::PlaceSuccessionAssertion;
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
        } => create_place(state, meta, place_id, human_id, place_type),
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
        PlaceCommand::AssertGeometry {
            place_id,
            geometry,
            date,
        } => assert_geometry(state, meta, place_id, geometry, date),
        PlaceCommand::AssertSuccession {
            place_id,
            from,
            to,
            kind,
            date,
        } => assert_succession(
            state,
            meta,
            refs,
            place_id,
            PlaceSuccessionAssertion { from, to, kind, date },
        ),
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
        attachment => decide_attachment(state, attachment, meta),
    }
}

/// Decides the plain attachment/metadata commands — those that simply require the place to exist
/// and (for a couple) pass a small within-aggregate check before emitting one event.
fn decide_attachment(
    state: &PlaceState,
    command: PlaceCommand,
    meta: &AssertionMeta,
) -> Result<Vec<PlaceEvent>, PlaceError> {
    let body = match command {
        PlaceCommand::SetCode { place_id, code } => {
            ensure_exists(state, place_id)?;
            if code.trim().is_empty() {
                return Err(PlaceError::EmptyCode);
            }
            PlaceEventBody::CodeSet { place_id, code }
        }
        PlaceCommand::AddCitation { place_id, citation_id } => {
            ensure_exists(state, place_id)?;
            PlaceEventBody::CitationAdded { place_id, citation_id }
        }
        PlaceCommand::AttachMedia { place_id, media } => {
            ensure_exists(state, place_id)?;
            PlaceEventBody::MediaAttached { place_id, media }
        }
        PlaceCommand::AttachNote { place_id, note_id } => {
            ensure_exists(state, place_id)?;
            PlaceEventBody::NoteAttached { place_id, note_id }
        }
        PlaceCommand::Tag { place_id, tag_id } => {
            ensure_exists(state, place_id)?;
            PlaceEventBody::Tagged { place_id, tag_id }
        }
        PlaceCommand::Untag { place_id, tag_id } => {
            ensure_exists(state, place_id)?;
            PlaceEventBody::Untagged { place_id, tag_id }
        }
        PlaceCommand::SetRestrictions { place_id, restrictions } => {
            ensure_exists(state, place_id)?;
            PlaceEventBody::RestrictionsChanged { place_id, restrictions }
        }
        PlaceCommand::SetHumanId { place_id, human_id } => {
            ensure_exists(state, place_id)?;
            place_human_id_changed(state, place_id, human_id)
        }
        // The lifecycle/dated-assertion/correction commands are handled by `decide`; they never
        // reach here.
        PlaceCommand::CreatePlace { .. }
        | PlaceCommand::SetPlaceType { .. }
        | PlaceCommand::AssertName { .. }
        | PlaceCommand::AssertEnclosedBy { .. }
        | PlaceCommand::AssertCoordinates { .. }
        | PlaceCommand::AssertGeometry { .. }
        | PlaceCommand::AssertSuccession { .. }
        | PlaceCommand::RetractAssertion { .. }
        | PlaceCommand::SupersedeAssertion { .. } => unreachable!("handled by decide"),
    };
    Ok(one(meta, body))
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: PlaceEventBody) -> Vec<PlaceEvent> {
    vec![PlaceEvent::new(meta, body)]
}

/// Decides `CreatePlace`: rejects a place that already exists, otherwise emits `PlaceCreated`.
fn create_place(
    state: &PlaceState,
    meta: &AssertionMeta,
    place_id: PlaceId,
    human_id: HumanId,
    place_type: PlaceType,
) -> Result<Vec<PlaceEvent>, PlaceError> {
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

/// Decides `AssertGeometry`: rejects a dangling place or an invalid ring, otherwise emits
/// `GeometryAsserted` (ADR 0024 — accumulates, unlike `AssertCoordinates`).
fn assert_geometry(
    state: &PlaceState,
    meta: &AssertionMeta,
    place_id: PlaceId,
    geometry: PlaceGeometry,
    date: Option<crate::date::GenealogicalDate>,
) -> Result<Vec<PlaceEvent>, PlaceError> {
    ensure_exists(state, place_id)?;
    if has_invalid_ring(&geometry) {
        return Err(PlaceError::InvalidGeometry);
    }
    Ok(one(
        meta,
        PlaceEventBody::GeometryAsserted {
            place_id,
            geometry,
            date,
        },
    ))
}

/// Decides `AssertSuccession`: rejects a dangling place, an empty `from`/`to`, an anchor not among
/// `from`, or a `from`/`to` place the projection does not know (the §9 aggregate-tax check via
/// `refs.missing_succession_place`), otherwise emits `SuccessionAsserted` (ADR 0026 — accumulates,
/// like `AssertGeometry`).
fn assert_succession(
    state: &PlaceState,
    meta: &AssertionMeta,
    refs: &PlaceRefs,
    place_id: PlaceId,
    assertion: PlaceSuccessionAssertion,
) -> Result<Vec<PlaceEvent>, PlaceError> {
    ensure_exists(state, place_id)?;
    if assertion.from.is_empty() || assertion.to.is_empty() {
        return Err(PlaceError::EmptySuccessionEndpoints);
    }
    if !assertion.from.contains(&place_id) {
        return Err(PlaceError::SuccessionAnchorMismatch(place_id));
    }
    if let Some(missing) = refs.missing_succession_place {
        return Err(PlaceError::UnknownPlace(missing));
    }
    Ok(one(
        meta,
        PlaceEventBody::SuccessionAsserted {
            place_id,
            from: assertion.from,
            to: assertion.to,
            kind: assertion.kind,
            date: assertion.date,
        },
    ))
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

/// Rejects a geometry whose exterior or any hole has fewer than 3 points (data-model §10.1
/// `InvalidGeometry`) — a `Point` never has a ring, so it always passes.
fn has_invalid_ring(geometry: &PlaceGeometry) -> bool {
    geometry.rings().iter().any(|ring| ring.len() < 3)
}

/// Rejects a command that targets a place which has not been created yet.
fn ensure_exists(state: &PlaceState, place_id: PlaceId) -> Result<(), PlaceError> {
    if state.exists {
        Ok(())
    } else {
        Err(PlaceError::NotFound(place_id))
    }
}

/// Folds `PlaceCreated`: seeds the aggregate's identity and its initial type assertion.
fn evolve_place_created(
    state: &mut PlaceState,
    event: &PlaceEvent,
    assertion_id: crate::ids::AssertionId,
    place_id: PlaceId,
    human_id: HumanId,
    place_type: crate::enums::PlaceType,
) {
    state.exists = true;
    state.place_id = Some(place_id);
    state.human_id = Some(human_id);
    state.place_type = Some(Attributed {
        assertion_id,
        value: Asserted::from_context(place_type, &event.context),
    });
    state.live_assertions.insert(assertion_id);
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut PlaceState, event: &PlaceEvent) {
    let assertion_id = event.assertion_id;
    match &event.body {
        PlaceEventBody::PlaceCreated {
            place_id,
            human_id,
            place_type,
        } => evolve_place_created(
            state,
            event,
            assertion_id,
            *place_id,
            human_id.clone(),
            place_type.clone(),
        ),
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
        PlaceEventBody::GeometryAsserted { geometry, date, .. } => {
            evolve_geometry_asserted(state, event, assertion_id, geometry.clone(), date.clone());
        }
        PlaceEventBody::SuccessionAsserted {
            from, to, kind, date, ..
        } => {
            evolve_succession_asserted(
                state,
                event,
                assertion_id,
                from.clone(),
                to.clone(),
                *kind,
                date.clone(),
            );
        }
        attachment => evolve_attachment(state, event, assertion_id, attachment),
    }
}

/// Folds the plain attachment/metadata/correction events — those that simply push, replace, or
/// remove one state entry, with no dated accumulation logic of their own.
fn evolve_attachment(
    state: &mut PlaceState,
    event: &PlaceEvent,
    assertion_id: crate::ids::AssertionId,
    body: &PlaceEventBody,
) {
    match body {
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
            state.restrictions_assertion = Some(assertion_id);
            state.live_assertions.insert(assertion_id);
        }
        PlaceEventBody::HumanIdChanged { human_id, .. } => {
            state.human_id = Some(human_id.clone());
        }
        PlaceEventBody::AssertionRetracted { target, .. } | PlaceEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
        }
        // The lifecycle/dated-assertion events are handled by `evolve`; they never reach here.
        PlaceEventBody::PlaceCreated { .. }
        | PlaceEventBody::PlaceTypeSet { .. }
        | PlaceEventBody::NameAsserted { .. }
        | PlaceEventBody::EnclosedByAsserted { .. }
        | PlaceEventBody::CoordinatesAsserted { .. }
        | PlaceEventBody::GeometryAsserted { .. }
        | PlaceEventBody::SuccessionAsserted { .. } => unreachable!("handled by evolve"),
    }
}

/// Folds `GeometryAsserted`: accumulates the dated shape (ADR 0024), unlike `coordinates`'
/// last-writer-wins.
fn evolve_geometry_asserted(
    state: &mut PlaceState,
    event: &PlaceEvent,
    assertion_id: crate::ids::AssertionId,
    geometry: PlaceGeometry,
    date: Option<crate::date::GenealogicalDate>,
) {
    let assertion = PlaceGeometryAssertion { geometry, date };
    state.geometries.push(Attributed {
        assertion_id,
        value: Asserted::from_context(assertion, &event.context),
    });
    state.live_assertions.insert(assertion_id);
}

/// Folds `SuccessionAsserted`: accumulates the dated identity change (ADR 0026), like `geometries`.
fn evolve_succession_asserted(
    state: &mut PlaceState,
    event: &PlaceEvent,
    assertion_id: crate::ids::AssertionId,
    from: Vec<PlaceId>,
    to: Vec<PlaceId>,
    kind: SuccessionKind,
    date: Option<crate::date::GenealogicalDate>,
) {
    let assertion = PlaceSuccessionAssertion { from, to, kind, date };
    state.successions.push(Attributed {
        assertion_id,
        value: Asserted::from_context(assertion, &event.context),
    });
    state.live_assertions.insert(assertion_id);
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

    const ENCLOSING_PRESENT: PlaceRefs = PlaceRefs {
        enclosing_exists: true,
        missing_succession_place: None,
    };
    const ENCLOSING_MISSING: PlaceRefs = PlaceRefs {
        enclosing_exists: false,
        missing_succession_place: None,
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
                confidence: Some(Confidence::Normal),
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

    fn geo_point(lat: i32, lon: i32) -> crate::geo::GeoCoordinates {
        GeoCoordinates {
            latitude: Microdegrees::from_microdegrees(lat),
            longitude: Microdegrees::from_microdegrees(lon),
        }
    }

    #[test]
    fn dated_geometries_accumulate_rather_than_replace() {
        let mut state = created_place(1);
        let boundary_1801 = decide(
            &state,
            PlaceCommand::AssertGeometry {
                place_id: place(1),
                geometry: crate::geo::PlaceGeometry::Polygon {
                    exterior: vec![
                        geo_point(60_000_000, 5_000_000),
                        geo_point(61_000_000, 5_000_000),
                        geo_point(61_000_000, 6_000_000),
                    ],
                    holes: Vec::new(),
                },
                date: Some(crate::date::GenealogicalDate {
                    calendar: crate::date::Calendar::Gregorian,
                    quality: crate::date::DateQuality::Normal,
                    modifier: crate::date::GenealogicalDateBody::Structured(crate::date::DateModifier::None(
                        crate::date::DatePoint {
                            year: Some(1801),
                            month: None,
                            day: None,
                        },
                    )),
                    time: None,
                    new_year_begins: None,
                    sort_value: 1801,
                    original_text: None,
                }),
            },
            &meta(2),
            &ENCLOSING_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &boundary_1801);
        assert_eq!(state.geometries.len(), 1);

        let boundary_1900 = decide(
            &state,
            PlaceCommand::AssertGeometry {
                place_id: place(1),
                geometry: crate::geo::PlaceGeometry::Point(geo_point(60_391_262, 5_322_054)),
                date: None,
            },
            &meta(3),
            &ENCLOSING_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &boundary_1900);
        assert_eq!(
            state.geometries.len(),
            2,
            "a second geometry assertion coexists with the first rather than replacing it"
        );
    }

    #[test]
    fn asserting_a_polygon_with_an_empty_exterior_ring_is_rejected() {
        let state = created_place(1);
        let err = decide(
            &state,
            PlaceCommand::AssertGeometry {
                place_id: place(1),
                geometry: crate::geo::PlaceGeometry::Polygon {
                    exterior: Vec::new(),
                    holes: Vec::new(),
                },
                date: None,
            },
            &meta(2),
            &ENCLOSING_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::InvalidGeometry);
    }

    #[test]
    fn asserting_a_polygon_with_a_too_small_hole_is_rejected() {
        let state = created_place(1);
        let exterior = vec![
            geo_point(60_000_000, 5_000_000),
            geo_point(61_000_000, 5_000_000),
            geo_point(61_000_000, 6_000_000),
        ];
        let err = decide(
            &state,
            PlaceCommand::AssertGeometry {
                place_id: place(1),
                geometry: crate::geo::PlaceGeometry::Polygon {
                    exterior,
                    holes: vec![vec![geo_point(60_300_000, 5_300_000), geo_point(60_400_000, 5_300_000)]],
                },
                date: None,
            },
            &meta(2),
            &ENCLOSING_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::InvalidGeometry);
    }

    #[test]
    fn asserting_a_geometry_against_a_dangling_place_is_not_found() {
        let state = PlaceState::default();
        let err = decide(
            &state,
            PlaceCommand::AssertGeometry {
                place_id: place(7),
                geometry: crate::geo::PlaceGeometry::Point(geo_point(60_391_262, 5_322_054)),
                date: None,
            },
            &meta(2),
            &ENCLOSING_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::NotFound(place(7)));
    }

    #[test]
    fn retracting_a_geometry_assertion_removes_it_non_destructively() {
        let mut state = created_place(1);
        let events = decide(
            &state,
            PlaceCommand::AssertGeometry {
                place_id: place(1),
                geometry: crate::geo::PlaceGeometry::Point(geo_point(60_391_262, 5_322_054)),
                date: None,
            },
            &meta(2),
            &ENCLOSING_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &events);
        let target = AssertionId::from_uuid(Uuid::from_u128(2));
        assert_eq!(state.geometries.len(), 1);

        let retract = decide(
            &state,
            PlaceCommand::RetractAssertion {
                place_id: place(1),
                target,
            },
            &meta(3),
            &ENCLOSING_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.geometries.is_empty());
        assert!(!state.live_assertions.contains(&target));
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

    #[test]
    fn retracting_a_restriction_change_clears_the_restrictions() {
        let mut state = created_place(1);
        let restrictions = std::collections::BTreeSet::from([crate::enums::Restriction::Locked]);
        let set = decide(
            &state,
            PlaceCommand::SetRestrictions {
                place_id: place(1),
                restrictions: restrictions.clone(),
            },
            &meta(2),
            &ENCLOSING_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &set);
        assert_eq!(state.restrictions, restrictions);

        let retract = decide(
            &state,
            PlaceCommand::RetractAssertion {
                place_id: place(1),
                target: crate::ids::AssertionId::from_uuid(uuid::Uuid::from_u128(2)),
            },
            &meta(3),
            &ENCLOSING_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.restrictions.is_empty(), "retracting the change clears the set");
        assert_eq!(state.restrictions_assertion, None);
    }

    const SUCCESSION_KNOWN: PlaceRefs = PlaceRefs {
        enclosing_exists: true,
        missing_succession_place: None,
    };

    fn succession_unknown(missing: PlaceId) -> PlaceRefs {
        PlaceRefs {
            enclosing_exists: true,
            missing_succession_place: Some(missing),
        }
    }

    #[test]
    fn a_merge_accumulates_as_a_succession_assertion() {
        let mut state = created_place(1);
        let events = decide(
            &state,
            PlaceCommand::AssertSuccession {
                place_id: place(1),
                from: vec![place(1), place(2)],
                to: vec![place(3)],
                kind: crate::enums::SuccessionKind::Merged,
                date: None,
            },
            &meta(2),
            &SUCCESSION_KNOWN,
        )
        .unwrap();
        apply_all(&mut state, &events);
        assert_eq!(state.successions.len(), 1);
        assert_eq!(state.successions[0].value.value.to, vec![place(3)]);
    }

    #[test]
    fn a_split_is_one_place_to_many() {
        let mut state = created_place(1);
        let events = decide(
            &state,
            PlaceCommand::AssertSuccession {
                place_id: place(1),
                from: vec![place(1)],
                to: vec![place(2), place(3)],
                kind: crate::enums::SuccessionKind::Split,
                date: None,
            },
            &meta(2),
            &SUCCESSION_KNOWN,
        )
        .unwrap();
        apply_all(&mut state, &events);
        assert_eq!(state.successions[0].value.value.to, vec![place(2), place(3)]);
    }

    #[test]
    fn successions_accumulate_rather_than_replace() {
        let mut state = created_place(1);
        let first = decide(
            &state,
            PlaceCommand::AssertSuccession {
                place_id: place(1),
                from: vec![place(1)],
                to: vec![place(2)],
                kind: crate::enums::SuccessionKind::Renamed,
                date: None,
            },
            &meta(2),
            &SUCCESSION_KNOWN,
        )
        .unwrap();
        apply_all(&mut state, &first);
        let second = decide(
            &state,
            PlaceCommand::AssertSuccession {
                place_id: place(1),
                from: vec![place(1)],
                to: vec![place(4)],
                kind: crate::enums::SuccessionKind::Absorbed,
                date: None,
            },
            &meta(3),
            &SUCCESSION_KNOWN,
        )
        .unwrap();
        apply_all(&mut state, &second);
        assert_eq!(
            state.successions.len(),
            2,
            "a second succession assertion coexists with the first rather than replacing it"
        );
    }

    #[test]
    fn an_empty_from_is_rejected() {
        let state = created_place(1);
        let err = decide(
            &state,
            PlaceCommand::AssertSuccession {
                place_id: place(1),
                from: Vec::new(),
                to: vec![place(2)],
                kind: crate::enums::SuccessionKind::Renamed,
                date: None,
            },
            &meta(2),
            &SUCCESSION_KNOWN,
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::EmptySuccessionEndpoints);
    }

    #[test]
    fn an_empty_to_is_rejected() {
        let state = created_place(1);
        let err = decide(
            &state,
            PlaceCommand::AssertSuccession {
                place_id: place(1),
                from: vec![place(1)],
                to: Vec::new(),
                kind: crate::enums::SuccessionKind::Renamed,
                date: None,
            },
            &meta(2),
            &SUCCESSION_KNOWN,
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::EmptySuccessionEndpoints);
    }

    #[test]
    fn an_anchor_not_among_from_is_rejected() {
        let state = created_place(1);
        let err = decide(
            &state,
            PlaceCommand::AssertSuccession {
                place_id: place(1),
                from: vec![place(2)],
                to: vec![place(3)],
                kind: crate::enums::SuccessionKind::Merged,
                date: None,
            },
            &meta(2),
            &SUCCESSION_KNOWN,
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::SuccessionAnchorMismatch(place(1)));
    }

    #[test]
    fn an_unknown_referenced_place_is_rejected() {
        let state = created_place(1);
        let err = decide(
            &state,
            PlaceCommand::AssertSuccession {
                place_id: place(1),
                from: vec![place(1)],
                to: vec![place(99)],
                kind: crate::enums::SuccessionKind::Renamed,
                date: None,
            },
            &meta(2),
            &succession_unknown(place(99)),
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::UnknownPlace(place(99)));
    }

    #[test]
    fn asserting_a_succession_against_a_dangling_place_is_not_found() {
        let state = PlaceState::default();
        let err = decide(
            &state,
            PlaceCommand::AssertSuccession {
                place_id: place(7),
                from: vec![place(7)],
                to: vec![place(8)],
                kind: crate::enums::SuccessionKind::Renamed,
                date: None,
            },
            &meta(2),
            &SUCCESSION_KNOWN,
        )
        .unwrap_err();
        assert_eq!(err, PlaceError::NotFound(place(7)));
    }

    #[test]
    fn retracting_a_succession_assertion_removes_it_non_destructively() {
        let mut state = created_place(1);
        let events = decide(
            &state,
            PlaceCommand::AssertSuccession {
                place_id: place(1),
                from: vec![place(1)],
                to: vec![place(2)],
                kind: crate::enums::SuccessionKind::Renamed,
                date: None,
            },
            &meta(2),
            &SUCCESSION_KNOWN,
        )
        .unwrap();
        apply_all(&mut state, &events);
        let target = AssertionId::from_uuid(Uuid::from_u128(2));
        assert_eq!(state.successions.len(), 1);

        let retract = decide(
            &state,
            PlaceCommand::RetractAssertion {
                place_id: place(1),
                target,
            },
            &meta(3),
            &SUCCESSION_KNOWN,
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.successions.is_empty());
        assert!(!state.live_assertions.contains(&target));
    }
}
