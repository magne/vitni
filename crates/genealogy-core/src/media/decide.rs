//! The pure Media decision core (ADR 0004 §3) and the `evolve` fold.

use crate::assertions::Attributed;
use crate::ids::MediaId;
use crate::media::command::MediaCommand;
use crate::media::error::MediaError;
use crate::media::event::{MediaEvent, MediaEventBody};
use crate::media::state::MediaState;
use crate::provenance::AssertionMeta;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`MediaError`] when the command violates a within-aggregate invariant: creating media
/// that exists, a command against absent media, or correcting an unknown assertion.
pub fn decide(state: &MediaState, command: MediaCommand, meta: &AssertionMeta) -> Result<Vec<MediaEvent>, MediaError> {
    match command {
        MediaCommand::CreateMedia { media_id, human_id } => {
            if state.exists {
                return Err(MediaError::AlreadyExists(media_id));
            }
            Ok(one(meta, MediaEventBody::MediaCreated { media_id, human_id }))
        }
        MediaCommand::SetPath { media_id, path } => {
            ensure_exists(state, media_id)?;
            Ok(one(meta, MediaEventBody::PathSet { media_id, path }))
        }
        MediaCommand::SetChecksum { media_id, checksum } => {
            ensure_exists(state, media_id)?;
            Ok(one(meta, MediaEventBody::ChecksumSet { media_id, checksum }))
        }
        MediaCommand::AssertDate { media_id, date } => {
            ensure_exists(state, media_id)?;
            Ok(one(meta, MediaEventBody::DateAsserted { media_id, date }))
        }
        MediaCommand::AddAttribute { media_id, attribute } => {
            ensure_exists(state, media_id)?;
            Ok(one(meta, MediaEventBody::AttributeAdded { media_id, attribute }))
        }
        MediaCommand::AddCitation { media_id, citation_id } => {
            ensure_exists(state, media_id)?;
            Ok(one(meta, MediaEventBody::CitationAdded { media_id, citation_id }))
        }
        MediaCommand::AttachNote { media_id, note_id } => {
            ensure_exists(state, media_id)?;
            Ok(one(meta, MediaEventBody::NoteAttached { media_id, note_id }))
        }
        MediaCommand::Tag { media_id, tag_id } => {
            ensure_exists(state, media_id)?;
            Ok(one(meta, MediaEventBody::Tagged { media_id, tag_id }))
        }
        MediaCommand::Untag { media_id, tag_id } => {
            ensure_exists(state, media_id)?;
            Ok(one(meta, MediaEventBody::Untagged { media_id, tag_id }))
        }
        MediaCommand::SetRestrictions { media_id, restrictions } => {
            ensure_exists(state, media_id)?;
            Ok(one(
                meta,
                MediaEventBody::RestrictionsChanged { media_id, restrictions },
            ))
        }
        MediaCommand::RetractAssertion { media_id, target } => {
            ensure_exists(state, media_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(MediaError::RetractsMissingAssertion(target));
            }
            Ok(one(meta, MediaEventBody::AssertionRetracted { media_id, target }))
        }
        MediaCommand::SupersedeAssertion {
            media_id,
            target,
            replacement,
        } => {
            ensure_exists(state, media_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(MediaError::SupersedesMissingAssertion(target));
            }
            let mut events = one(meta, MediaEventBody::AssertionSuperseded { media_id, target });
            events.extend(decide(state, *replacement, meta)?);
            Ok(events)
        }
    }
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: MediaEventBody) -> Vec<MediaEvent> {
    vec![MediaEvent::new(meta, body)]
}

/// Rejects a command that targets media which has not been created yet.
fn ensure_exists(state: &MediaState, media_id: MediaId) -> Result<(), MediaError> {
    if state.exists {
        Ok(())
    } else {
        Err(MediaError::NotFound(media_id))
    }
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut MediaState, event: &MediaEvent) {
    let assertion_id = event.assertion_id;
    match &event.body {
        MediaEventBody::MediaCreated { media_id, human_id } => {
            state.exists = true;
            state.media_id = Some(*media_id);
            state.human_id = Some(human_id.clone());
            state.live_assertions.insert(assertion_id);
        }
        MediaEventBody::PathSet { path, .. } => {
            state.path = Some(Attributed {
                assertion_id,
                value: path.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        MediaEventBody::ChecksumSet { checksum, .. } => {
            state.checksum = Some(Attributed {
                assertion_id,
                value: checksum.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        MediaEventBody::DateAsserted { date, .. } => {
            state.date = Some(Attributed {
                assertion_id,
                value: date.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        MediaEventBody::AttributeAdded { attribute, .. } => {
            state.attributes.push(Attributed {
                assertion_id,
                value: attribute.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        MediaEventBody::CitationAdded { .. }
        | MediaEventBody::NoteAttached { .. }
        | MediaEventBody::Tagged { .. }
        | MediaEventBody::Untagged { .. } => {
            state.live_assertions.insert(assertion_id);
        }
        MediaEventBody::RestrictionsChanged { restrictions, .. } => {
            state.restrictions.clone_from(restrictions);
            state.live_assertions.insert(assertion_id);
        }
        MediaEventBody::AssertionRetracted { target, .. } | MediaEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::ids::{AgentId, AssertionId, HumanId, MediaId};
    use crate::media::command::MediaCommand;
    use crate::media::error::MediaError;
    use crate::media::state::MediaState;
    use crate::media_path::MediaPath;
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use time::macros::datetime;
    use uuid::Uuid;

    fn media(n: u128) -> MediaId {
        MediaId::from_uuid(Uuid::from_u128(n))
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

    fn apply_all(state: &mut MediaState, events: &[crate::media::event::MediaEvent]) {
        for event in events {
            evolve(state, event);
        }
    }

    fn created_media(id: u128) -> MediaState {
        let mut state = MediaState::default();
        let events = decide(
            &state,
            MediaCommand::CreateMedia {
                media_id: media(id),
                human_id: HumanId::new("M1"),
            },
            &meta(1),
        )
        .unwrap();
        apply_all(&mut state, &events);
        state
    }

    #[test]
    fn command_against_absent_media_is_not_found() {
        let state = MediaState::default();
        let err = decide(
            &state,
            MediaCommand::SetChecksum {
                media_id: media(7),
                checksum: "abc".to_owned(),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, MediaError::NotFound(media(7)));
    }

    #[test]
    fn path_is_last_writer_wins_and_retract_removes_it() {
        let mut state = created_media(1);
        let set = decide(
            &state,
            MediaCommand::SetPath {
                media_id: media(1),
                path: MediaPath::File("photos/ada.jpg".to_owned()),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &set);
        assert!(state.path.is_some());

        let target = AssertionId::from_uuid(Uuid::from_u128(2));
        let retract = decide(
            &state,
            MediaCommand::RetractAssertion {
                media_id: media(1),
                target,
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.path.is_none());
    }
}
