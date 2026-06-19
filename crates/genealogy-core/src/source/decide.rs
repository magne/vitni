//! The pure Source decision core (ADR 0004 §3) and the `evolve` fold.

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
/// source that exists, or a command against an absent source.
pub fn decide(
    state: &SourceState,
    command: SourceCommand,
    meta: &AssertionMeta,
) -> Result<Vec<SourceEvent>, SourceError> {
    let body = match command {
        SourceCommand::CreateSource { source_id, human_id } => {
            if state.exists {
                return Err(SourceError::AlreadyExists(source_id));
            }
            SourceEventBody::SourceCreated { source_id, human_id }
        }
        SourceCommand::SetTitle { source_id, title } => {
            ensure_exists(state, source_id)?;
            SourceEventBody::TitleSet { source_id, title }
        }
    };
    Ok(vec![SourceEvent::new(meta, body)])
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
    match &event.body {
        SourceEventBody::SourceCreated { source_id, human_id } => {
            state.exists = true;
            state.source_id = Some(*source_id);
            state.human_id = Some(human_id.clone());
        }
        SourceEventBody::TitleSet { title, .. } => {
            state.title = Some(title.clone());
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
        assert_eq!(state.title.as_deref(), Some("Folketelling 1801"));
    }
}
