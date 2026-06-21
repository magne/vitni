//! The pure Note decision core (ADR 0004 §3) and the `evolve` fold.

use crate::assertions::Attributed;
use crate::ids::NoteId;
use crate::note::command::NoteCommand;
use crate::note::error::NoteError;
use crate::note::event::{NoteEvent, NoteEventBody};
use crate::note::state::NoteState;
use crate::provenance::AssertionMeta;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`NoteError`] when the command violates a within-aggregate invariant: creating a note
/// that exists, a command against an absent note, or correcting an unknown assertion.
pub fn decide(state: &NoteState, command: NoteCommand, meta: &AssertionMeta) -> Result<Vec<NoteEvent>, NoteError> {
    match command {
        NoteCommand::CreateNote { note_id, human_id } => {
            if state.exists {
                return Err(NoteError::AlreadyExists(note_id));
            }
            Ok(one(meta, NoteEventBody::NoteCreated { note_id, human_id }))
        }
        NoteCommand::SetNoteType { note_id, note_type } => {
            ensure_exists(state, note_id)?;
            Ok(one(meta, NoteEventBody::NoteTypeSet { note_id, note_type }))
        }
        NoteCommand::SetRichText { note_id, text } => {
            ensure_exists(state, note_id)?;
            Ok(one(meta, NoteEventBody::RichTextSet { note_id, text }))
        }
        NoteCommand::Tag { note_id, tag_id } => {
            ensure_exists(state, note_id)?;
            Ok(one(meta, NoteEventBody::Tagged { note_id, tag_id }))
        }
        NoteCommand::Untag { note_id, tag_id } => {
            ensure_exists(state, note_id)?;
            Ok(one(meta, NoteEventBody::Untagged { note_id, tag_id }))
        }
        NoteCommand::RetractAssertion { note_id, target } => {
            ensure_exists(state, note_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(NoteError::RetractsMissingAssertion(target));
            }
            Ok(one(meta, NoteEventBody::AssertionRetracted { note_id, target }))
        }
        NoteCommand::SupersedeAssertion {
            note_id,
            target,
            replacement,
        } => {
            ensure_exists(state, note_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(NoteError::SupersedesMissingAssertion(target));
            }
            let mut events = one(meta, NoteEventBody::AssertionSuperseded { note_id, target });
            events.extend(decide(state, *replacement, meta)?);
            Ok(events)
        }
    }
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: NoteEventBody) -> Vec<NoteEvent> {
    vec![NoteEvent::new(meta, body)]
}

/// Rejects a command that targets a note which has not been created yet.
fn ensure_exists(state: &NoteState, note_id: NoteId) -> Result<(), NoteError> {
    if state.exists {
        Ok(())
    } else {
        Err(NoteError::NotFound(note_id))
    }
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut NoteState, event: &NoteEvent) {
    let assertion_id = event.assertion_id;
    match &event.body {
        NoteEventBody::NoteCreated { note_id, human_id } => {
            state.exists = true;
            state.note_id = Some(*note_id);
            state.human_id = Some(human_id.clone());
            state.live_assertions.insert(assertion_id);
        }
        NoteEventBody::NoteTypeSet { note_type, .. } => {
            state.note_type = Some(Attributed {
                assertion_id,
                value: note_type.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        NoteEventBody::RichTextSet { text, .. } => {
            state.text = Some(Attributed {
                assertion_id,
                value: text.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        NoteEventBody::Tagged { .. } | NoteEventBody::Untagged { .. } => {
            state.live_assertions.insert(assertion_id);
        }
        NoteEventBody::AssertionRetracted { target, .. } | NoteEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::ids::{AgentId, AssertionId, HumanId, NoteId};
    use crate::note::command::NoteCommand;
    use crate::note::error::NoteError;
    use crate::note::event::NoteEventBody;
    use crate::note::state::NoteState;
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use crate::text::{MediaType, RichText};
    use time::macros::datetime;
    use uuid::Uuid;

    fn note(n: u128) -> NoteId {
        NoteId::from_uuid(Uuid::from_u128(n))
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

    fn apply_all(state: &mut NoteState, events: &[crate::note::event::NoteEvent]) {
        for event in events {
            evolve(state, event);
        }
    }

    fn created_note(id: u128) -> NoteState {
        let mut state = NoteState::default();
        let events = decide(
            &state,
            NoteCommand::CreateNote {
                note_id: note(id),
                human_id: HumanId::new("N1"),
            },
            &meta(1),
        )
        .unwrap();
        apply_all(&mut state, &events);
        state
    }

    #[test]
    fn command_against_absent_note_is_not_found() {
        let state = NoteState::default();
        let err = decide(
            &state,
            NoteCommand::SetRichText {
                note_id: note(7),
                text: RichText {
                    text: "x".to_owned(),
                    media_type: MediaType::Markdown,
                    language: None,
                },
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, NoteError::NotFound(note(7)));
    }

    #[test]
    fn rich_text_is_last_writer_wins_and_retract_removes_it() {
        let mut state = created_note(1);
        let set = decide(
            &state,
            NoteCommand::SetRichText {
                note_id: note(1),
                text: RichText {
                    text: "Born in Bergen.".to_owned(),
                    media_type: MediaType::Markdown,
                    language: None,
                },
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &set);
        assert!(state.text.is_some());

        let target = AssertionId::from_uuid(Uuid::from_u128(2));
        let retract = decide(
            &state,
            NoteCommand::RetractAssertion {
                note_id: note(1),
                target,
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.text.is_none());
        assert!(matches!(set[0].body, NoteEventBody::RichTextSet { .. }));
    }
}
