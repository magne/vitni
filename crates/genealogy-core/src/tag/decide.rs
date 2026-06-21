//! The pure Tag decision core (ADR 0004 §3) and the `evolve` fold.

use crate::ids::TagId;
use crate::provenance::AssertionMeta;
use crate::tag::command::TagCommand;
use crate::tag::error::TagError;
use crate::tag::event::{TagEvent, TagEventBody};
use crate::tag::state::TagState;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`TagError`] when the command violates a within-aggregate invariant: creating a tag
/// that exists, a command against an absent tag, or an empty name.
pub fn decide(state: &TagState, command: TagCommand, meta: &AssertionMeta) -> Result<Vec<TagEvent>, TagError> {
    match command {
        TagCommand::CreateTag { tag_id, name } => {
            if state.exists {
                return Err(TagError::AlreadyExists(tag_id));
            }
            if name.trim().is_empty() {
                return Err(TagError::EmptyName);
            }
            Ok(one(meta, TagEventBody::TagCreated { tag_id, name }))
        }
        TagCommand::RenameTag { tag_id, name } => {
            ensure_exists(state, tag_id)?;
            if name.trim().is_empty() {
                return Err(TagError::EmptyName);
            }
            Ok(one(meta, TagEventBody::TagRenamed { tag_id, name }))
        }
        TagCommand::SetTagColor { tag_id, color } => {
            ensure_exists(state, tag_id)?;
            Ok(one(meta, TagEventBody::TagColorSet { tag_id, color }))
        }
        TagCommand::SetTagPriority { tag_id, priority } => {
            ensure_exists(state, tag_id)?;
            Ok(one(meta, TagEventBody::TagPrioritySet { tag_id, priority }))
        }
    }
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: TagEventBody) -> Vec<TagEvent> {
    vec![TagEvent::new(meta, body)]
}

/// Rejects a command that targets a tag which has not been created yet.
fn ensure_exists(state: &TagState, tag_id: TagId) -> Result<(), TagError> {
    if state.exists {
        Ok(())
    } else {
        Err(TagError::NotFound(tag_id))
    }
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut TagState, event: &TagEvent) {
    match &event.body {
        TagEventBody::TagCreated { tag_id, name } => {
            state.exists = true;
            state.tag_id = Some(*tag_id);
            state.name = Some(name.clone());
        }
        TagEventBody::TagRenamed { name, .. } => {
            state.name = Some(name.clone());
        }
        TagEventBody::TagColorSet { color, .. } => {
            state.color = Some(color.clone());
        }
        TagEventBody::TagPrioritySet { priority, .. } => {
            state.priority = Some(*priority);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::ids::{AgentId, AssertionId, TagId};
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use crate::tag::command::TagCommand;
    use crate::tag::error::TagError;
    use crate::tag::state::TagState;
    use time::macros::datetime;
    use uuid::Uuid;

    fn tag(n: u128) -> TagId {
        TagId::from_uuid(Uuid::from_u128(n))
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

    fn apply_all(state: &mut TagState, events: &[crate::tag::event::TagEvent]) {
        for event in events {
            evolve(state, event);
        }
    }

    fn created_tag(id: u128) -> TagState {
        let mut state = TagState::default();
        let events = decide(
            &state,
            TagCommand::CreateTag {
                tag_id: tag(id),
                name: "Direct line".to_owned(),
            },
            &meta(1),
        )
        .unwrap();
        apply_all(&mut state, &events);
        state
    }

    #[test]
    fn creating_with_an_empty_name_is_rejected() {
        let state = TagState::default();
        let err = decide(
            &state,
            TagCommand::CreateTag {
                tag_id: tag(1),
                name: "   ".to_owned(),
            },
            &meta(1),
        )
        .unwrap_err();
        assert_eq!(err, TagError::EmptyName);
    }

    #[test]
    fn recreating_is_rejected_and_setters_are_last_writer_wins() {
        let mut state = created_tag(1);
        let err = decide(
            &state,
            TagCommand::CreateTag {
                tag_id: tag(1),
                name: "x".to_owned(),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, TagError::AlreadyExists(tag(1)));

        for command in [
            TagCommand::RenameTag {
                tag_id: tag(1),
                name: "Maternal line".to_owned(),
            },
            TagCommand::SetTagColor {
                tag_id: tag(1),
                color: "#1f77b4".to_owned(),
            },
            TagCommand::SetTagPriority {
                tag_id: tag(1),
                priority: 5,
            },
        ] {
            let events = decide(&state, command, &meta(3)).unwrap();
            apply_all(&mut state, &events);
        }
        assert_eq!(state.name.as_deref(), Some("Maternal line"));
        assert_eq!(state.color.as_deref(), Some("#1f77b4"));
        assert_eq!(state.priority, Some(5));
    }

    #[test]
    fn command_against_absent_tag_is_not_found() {
        let state = TagState::default();
        let err = decide(
            &state,
            TagCommand::SetTagPriority {
                tag_id: tag(7),
                priority: 1,
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, TagError::NotFound(tag(7)));
    }
}
