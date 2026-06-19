//! The pure Citation decision core (ADR 0004 §3) and the `evolve` fold.
//!
//! `decide(state, command, meta, refs) -> Result<Vec<CitationEvent>, CitationError>` reads no clock
//! and generates no id, and does not read another aggregate's projection itself: the
//! cross-aggregate facts arrive in `refs`, resolved by the `Services`-backed adapter
//! ([`crate::citation::aggregate`]) from the [`CitationRefResolver`](super::ref_resolver). So the
//! rule (`UnknownSource`) lives here, in the pure core, while the impure read stays at the edge.

use crate::citation::command::CitationCommand;
use crate::citation::error::CitationError;
use crate::citation::event::{CitationEvent, CitationEventBody};
use crate::citation::ref_resolver::CitationRefs;
use crate::citation::state::CitationState;
use crate::ids::CitationId;
use crate::provenance::AssertionMeta;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`CitationError`] when the command violates an invariant: creating a citation that
/// exists, a command against an absent citation, or — the §9 aggregate-tax check — creating a
/// citation against a source the projection does not know (`refs.source_exists == false`).
pub fn decide(
    state: &CitationState,
    command: CitationCommand,
    meta: &AssertionMeta,
    refs: &CitationRefs,
) -> Result<Vec<CitationEvent>, CitationError> {
    let body = match command {
        CitationCommand::CreateCitation {
            citation_id,
            human_id,
            source_id,
        } => {
            if state.exists {
                return Err(CitationError::AlreadyExists(citation_id));
            }
            if !refs.source_exists {
                return Err(CitationError::UnknownSource(source_id));
            }
            CitationEventBody::CitationCreated {
                citation_id,
                human_id,
                source_id,
            }
        }
        CitationCommand::SetPage { citation_id, page } => {
            ensure_exists(state, citation_id)?;
            CitationEventBody::PageSet { citation_id, page }
        }
    };
    Ok(vec![CitationEvent::new(meta, body)])
}

/// Rejects a command that targets a citation which has not been created yet.
fn ensure_exists(state: &CitationState, citation_id: CitationId) -> Result<(), CitationError> {
    if state.exists {
        Ok(())
    } else {
        Err(CitationError::NotFound(citation_id))
    }
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut CitationState, event: &CitationEvent) {
    match &event.body {
        CitationEventBody::CitationCreated {
            citation_id,
            human_id,
            source_id,
        } => {
            state.exists = true;
            state.citation_id = Some(*citation_id);
            state.human_id = Some(human_id.clone());
            state.source_id = Some(*source_id);
        }
        CitationEventBody::PageSet { page, .. } => {
            state.page = Some(page.clone());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::citation::command::CitationCommand;
    use crate::citation::error::CitationError;
    use crate::citation::event::CitationEventBody;
    use crate::citation::ref_resolver::CitationRefs;
    use crate::citation::state::CitationState;
    use crate::ids::{AgentId, AssertionId, CitationId, HumanId, SourceId};
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use time::macros::datetime;
    use uuid::Uuid;

    fn citation(n: u128) -> CitationId {
        CitationId::from_uuid(Uuid::from_u128(n))
    }

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

    const SOURCE_PRESENT: CitationRefs = CitationRefs { source_exists: true };
    const SOURCE_MISSING: CitationRefs = CitationRefs { source_exists: false };

    fn apply_all(state: &mut CitationState, events: &[crate::citation::event::CitationEvent]) {
        for event in events {
            evolve(state, event);
        }
    }

    fn created_citation(id: u128) -> CitationState {
        let mut state = CitationState::default();
        let events = decide(
            &state,
            CitationCommand::CreateCitation {
                citation_id: citation(id),
                human_id: HumanId::new("C1"),
                source_id: source(1),
            },
            &meta(1),
            &SOURCE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &events);
        state
    }

    #[test]
    fn create_citation_against_a_present_source_emits_citation_created() {
        let state = CitationState::default();
        let events = decide(
            &state,
            CitationCommand::CreateCitation {
                citation_id: citation(1),
                human_id: HumanId::new("C1"),
                source_id: source(1),
            },
            &meta(1),
            &SOURCE_PRESENT,
        )
        .unwrap();
        assert!(matches!(events[0].body, CitationEventBody::CitationCreated { .. }));
    }

    #[test]
    fn create_citation_against_a_missing_source_is_unknown_source() {
        // The aggregate-tax check: the resolver reported the cited source absent, so `decide`
        // rejects with the domain error (proving the Services path, not an app-layer guard).
        let state = CitationState::default();
        let err = decide(
            &state,
            CitationCommand::CreateCitation {
                citation_id: citation(1),
                human_id: HumanId::new("C1"),
                source_id: source(99),
            },
            &meta(1),
            &SOURCE_MISSING,
        )
        .unwrap_err();
        assert_eq!(err, CitationError::UnknownSource(source(99)));
    }

    #[test]
    fn recreating_an_existing_citation_is_rejected() {
        let state = created_citation(1);
        let err = decide(
            &state,
            CitationCommand::CreateCitation {
                citation_id: citation(1),
                human_id: HumanId::new("C1"),
                source_id: source(1),
            },
            &meta(2),
            &SOURCE_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, CitationError::AlreadyExists(citation(1)));
    }

    #[test]
    fn setting_a_page_on_an_absent_citation_is_not_found() {
        let state = CitationState::default();
        let err = decide(
            &state,
            CitationCommand::SetPage {
                citation_id: citation(7),
                page: "p. 42".to_owned(),
            },
            &meta(2),
            &SOURCE_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, CitationError::NotFound(citation(7)));
    }

    #[test]
    fn setting_a_page_records_it_last_writer_wins() {
        let mut state = created_citation(1);
        for (assertion, page) in [(2, "p. 1"), (3, "p. 42")] {
            let events = decide(
                &state,
                CitationCommand::SetPage {
                    citation_id: citation(1),
                    page: page.to_owned(),
                },
                &meta(assertion),
                &SOURCE_PRESENT,
            )
            .unwrap();
            apply_all(&mut state, &events);
        }
        assert_eq!(state.page.as_deref(), Some("p. 42"));
    }
}
