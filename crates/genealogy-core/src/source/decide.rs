//! The pure Source decision core (ADR 0004 §3) and the `evolve` fold.

use crate::assertions::Attributed;
use crate::ids::SourceId;
use crate::provenance::AssertionMeta;
use crate::source::command::SourceCommand;
use crate::source::error::SourceError;
use crate::source::event::{SourceEvent, SourceEventBody};
use crate::source::state::SourceState;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`SourceError`] when the command violates a within-aggregate invariant: creating a
/// source that exists, a command against an absent source, or correcting an unknown assertion.
pub fn decide(
    state: &SourceState,
    command: SourceCommand,
    meta: &AssertionMeta,
) -> Result<Vec<SourceEvent>, SourceError> {
    match command {
        SourceCommand::CreateSource { source_id, human_id } => {
            if state.exists {
                return Err(SourceError::AlreadyExists(source_id));
            }
            Ok(one(meta, SourceEventBody::SourceCreated { source_id, human_id }))
        }
        SourceCommand::SetTitle { source_id, title } => {
            ensure_exists(state, source_id)?;
            Ok(one(meta, SourceEventBody::TitleSet { source_id, title }))
        }
        SourceCommand::RetractAssertion { source_id, target } => {
            ensure_exists(state, source_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(SourceError::RetractsMissingAssertion(target));
            }
            Ok(one(meta, SourceEventBody::AssertionRetracted { source_id, target }))
        }
        SourceCommand::SupersedeAssertion {
            source_id,
            target,
            replacement,
        } => {
            ensure_exists(state, source_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(SourceError::SupersedesMissingAssertion(target));
            }
            let mut events = one(meta, SourceEventBody::AssertionSuperseded { source_id, target });
            events.extend(decide(state, *replacement, meta)?);
            Ok(events)
        }
    }
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: SourceEventBody) -> Vec<SourceEvent> {
    vec![SourceEvent::new(meta, body)]
}

/// Rejects a command that targets a source which has not been created yet.
fn ensure_exists(state: &SourceState, source_id: SourceId) -> Result<(), SourceError> {
    if state.exists {
        Ok(())
    } else {
        Err(SourceError::NotFound(source_id))
    }
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut SourceState, event: &SourceEvent) {
    let assertion_id = event.assertion_id;
    match &event.body {
        SourceEventBody::SourceCreated { source_id, human_id } => {
            state.exists = true;
            state.source_id = Some(*source_id);
            state.human_id = Some(human_id.clone());
            state.live_assertions.insert(assertion_id);
        }
        SourceEventBody::TitleSet { title, .. } => {
            state.title = Some(Attributed {
                assertion_id,
                value: title.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        SourceEventBody::AssertionRetracted { target, .. } | SourceEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::ids::{AgentId, AssertionId, HumanId, SourceId};
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use crate::source::command::SourceCommand;
    use crate::source::error::SourceError;
    use crate::source::event::SourceEventBody;
    use crate::source::state::SourceState;
    use time::macros::datetime;
    use uuid::Uuid;

    fn source(n: u128) -> SourceId {
        SourceId::from_uuid(Uuid::from_u128(n))
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

    fn apply_all(state: &mut SourceState, events: &[crate::source::event::SourceEvent]) {
        for event in events {
            evolve(state, event);
        }
    }

    fn created_source(id: u128) -> SourceState {
        let mut state = SourceState::default();
        let events = decide(
            &state,
            SourceCommand::CreateSource {
                source_id: source(id),
                human_id: HumanId::new("S1"),
            },
            &meta(1),
        )
        .unwrap();
        apply_all(&mut state, &events);
        state
    }

    #[test]
    fn create_source_on_empty_state_emits_source_created() {
        let state = SourceState::default();
        let events = decide(
            &state,
            SourceCommand::CreateSource {
                source_id: source(1),
                human_id: HumanId::new("S1"),
            },
            &meta(1),
        )
        .unwrap();
        assert_eq!(events.len(), 1);
        assert!(matches!(events[0].body, SourceEventBody::SourceCreated { .. }));
    }

    #[test]
    fn recreating_an_existing_source_is_rejected() {
        let state = created_source(1);
        let err = decide(
            &state,
            SourceCommand::CreateSource {
                source_id: source(1),
                human_id: HumanId::new("S1"),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, SourceError::AlreadyExists(source(1)));
    }

    #[test]
    fn setting_a_title_on_an_absent_source_is_not_found() {
        let state = SourceState::default();
        let err = decide(
            &state,
            SourceCommand::SetTitle {
                source_id: source(7),
                title: "Folketelling 1801".to_owned(),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, SourceError::NotFound(source(7)));
    }

    #[test]
    fn setting_a_title_records_it_last_writer_wins() {
        let mut state = created_source(1);
        for (assertion, title) in [(2, "Draft"), (3, "Folketelling 1801")] {
            let events = decide(
                &state,
                SourceCommand::SetTitle {
                    source_id: source(1),
                    title: title.to_owned(),
                },
                &meta(assertion),
            )
            .unwrap();
            apply_all(&mut state, &events);
        }
        assert_eq!(
            state.title.as_ref().map(|t| t.value.as_str()),
            Some("Folketelling 1801")
        );
    }

    #[test]
    fn retracting_an_unknown_assertion_is_rejected() {
        let state = created_source(1);
        let unknown = AssertionId::from_uuid(Uuid::from_u128(999));
        let err = decide(
            &state,
            SourceCommand::RetractAssertion {
                source_id: source(1),
                target: unknown,
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, SourceError::RetractsMissingAssertion(unknown));
    }

    #[test]
    fn retracting_a_live_title_removes_it_non_destructively() {
        let mut state = created_source(1);
        let title_events = decide(
            &state,
            SourceCommand::SetTitle {
                source_id: source(1),
                title: "Draft".to_owned(),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &title_events);
        let title_assertion = AssertionId::from_uuid(Uuid::from_u128(2));
        assert!(state.title.is_some());

        let retract = decide(
            &state,
            SourceCommand::RetractAssertion {
                source_id: source(1),
                target: title_assertion,
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &retract);

        assert!(state.title.is_none());
        assert!(!state.live_assertions.contains(&title_assertion));
    }

    #[test]
    fn superseding_emits_a_supersession_then_the_replacement_event() {
        let mut state = created_source(1);
        let first = decide(
            &state,
            SourceCommand::SetTitle {
                source_id: source(1),
                title: "Draft".to_owned(),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &first);
        let target = AssertionId::from_uuid(Uuid::from_u128(2));

        let events = decide(
            &state,
            SourceCommand::SupersedeAssertion {
                source_id: source(1),
                target,
                replacement: Box::new(SourceCommand::SetTitle {
                    source_id: source(1),
                    title: "Folketelling 1801".to_owned(),
                }),
            },
            &meta(3),
        )
        .unwrap();

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].body, SourceEventBody::AssertionSuperseded { .. }));
        assert!(matches!(events[1].body, SourceEventBody::TitleSet { .. }));

        apply_all(&mut state, &events);
        assert_eq!(
            state.title.as_ref().map(|t| t.value.as_str()),
            Some("Folketelling 1801")
        );
        assert!(!state.live_assertions.contains(&target));
    }
}
