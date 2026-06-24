//! The pure Repository decision core (ADR 0004 §3) and the `evolve` fold.

use crate::assertions::Attributed;
use crate::ids::{AssertionId, RepositoryId};
use crate::provenance::AssertionMeta;
use crate::repository::command::RepositoryCommand;
use crate::repository::error::RepositoryError;
use crate::repository::event::{RepositoryEvent, RepositoryEventBody};
use crate::repository::state::RepositoryState;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`RepositoryError`] when the command violates a within-aggregate invariant: creating a
/// repository that exists, a command against an absent repository, an empty name, or correcting an
/// unknown assertion.
pub fn decide(
    state: &RepositoryState,
    command: RepositoryCommand,
    meta: &AssertionMeta,
) -> Result<Vec<RepositoryEvent>, RepositoryError> {
    match command {
        RepositoryCommand::CreateRepository {
            repository_id,
            human_id,
        } => {
            if state.exists {
                return Err(RepositoryError::AlreadyExists(repository_id));
            }
            Ok(one(
                meta,
                RepositoryEventBody::RepositoryCreated {
                    repository_id,
                    human_id,
                },
            ))
        }
        RepositoryCommand::SetRepositoryType {
            repository_id,
            repository_type,
        } => {
            ensure_exists(state, repository_id)?;
            Ok(one(
                meta,
                RepositoryEventBody::RepositoryTypeSet {
                    repository_id,
                    repository_type,
                },
            ))
        }
        RepositoryCommand::SetName { repository_id, name } => {
            ensure_exists(state, repository_id)?;
            if name.trim().is_empty() {
                return Err(RepositoryError::EmptyName);
            }
            Ok(one(meta, RepositoryEventBody::NameSet { repository_id, name }))
        }
        RepositoryCommand::AddAddress { repository_id, address } => {
            ensure_exists(state, repository_id)?;
            Ok(one(meta, RepositoryEventBody::AddressAdded { repository_id, address }))
        }
        RepositoryCommand::AddUrl { repository_id, url } => {
            ensure_exists(state, repository_id)?;
            Ok(one(meta, RepositoryEventBody::UrlAdded { repository_id, url }))
        }
        RepositoryCommand::AttachNote { repository_id, note_id } => {
            ensure_exists(state, repository_id)?;
            Ok(one(meta, RepositoryEventBody::NoteAttached { repository_id, note_id }))
        }
        RepositoryCommand::Tag { repository_id, tag_id } => {
            ensure_exists(state, repository_id)?;
            Ok(one(meta, RepositoryEventBody::Tagged { repository_id, tag_id }))
        }
        RepositoryCommand::Untag { repository_id, tag_id } => {
            ensure_exists(state, repository_id)?;
            Ok(one(meta, RepositoryEventBody::Untagged { repository_id, tag_id }))
        }
        RepositoryCommand::SetRestrictions {
            repository_id,
            restrictions,
        } => {
            ensure_exists(state, repository_id)?;
            Ok(one(
                meta,
                RepositoryEventBody::RestrictionsChanged {
                    repository_id,
                    restrictions,
                },
            ))
        }
        RepositoryCommand::RetractAssertion { repository_id, target } => {
            ensure_exists(state, repository_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(RepositoryError::RetractsMissingAssertion(target));
            }
            Ok(one(
                meta,
                RepositoryEventBody::AssertionRetracted { repository_id, target },
            ))
        }
        RepositoryCommand::SupersedeAssertion {
            repository_id,
            target,
            replacement,
        } => {
            ensure_exists(state, repository_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(RepositoryError::SupersedesMissingAssertion(target));
            }
            let mut events = one(meta, RepositoryEventBody::AssertionSuperseded { repository_id, target });
            events.extend(decide(state, *replacement, meta)?);
            Ok(events)
        }
    }
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: RepositoryEventBody) -> Vec<RepositoryEvent> {
    vec![RepositoryEvent::new(meta, body)]
}

/// Rejects a command that targets a repository which has not been created yet.
fn ensure_exists(state: &RepositoryState, repository_id: RepositoryId) -> Result<(), RepositoryError> {
    if state.exists {
        Ok(())
    } else {
        Err(RepositoryError::NotFound(repository_id))
    }
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut RepositoryState, event: &RepositoryEvent) {
    let assertion_id = event.assertion_id;
    match &event.body {
        RepositoryEventBody::RepositoryCreated {
            repository_id,
            human_id,
        } => {
            state.exists = true;
            state.repository_id = Some(*repository_id);
            state.human_id = Some(human_id.clone());
            state.live_assertions.insert(assertion_id);
        }
        RepositoryEventBody::RepositoryTypeSet { repository_type, .. } => {
            state.repository_type = Some(Attributed {
                assertion_id,
                value: repository_type.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        RepositoryEventBody::NameSet { name, .. } => {
            state.name = Some(Attributed {
                assertion_id,
                value: name.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        RepositoryEventBody::AddressAdded { address, .. } => {
            state.addresses.push(Attributed {
                assertion_id,
                value: address.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        RepositoryEventBody::UrlAdded { url, .. } => {
            state.urls.push(Attributed {
                assertion_id,
                value: url.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        RepositoryEventBody::NoteAttached { .. }
        | RepositoryEventBody::Tagged { .. }
        | RepositoryEventBody::Untagged { .. } => {
            fold_attachment(state, assertion_id, &event.body);
            state.live_assertions.insert(assertion_id);
        }
        RepositoryEventBody::RestrictionsChanged { restrictions, .. } => {
            state.restrictions.clone_from(restrictions);
            state.live_assertions.insert(assertion_id);
        }
        RepositoryEventBody::AssertionRetracted { target, .. }
        | RepositoryEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
        }
    }
}

/// Folds an attachment event (note/tag) into the projected state.
fn fold_attachment(state: &mut RepositoryState, assertion_id: AssertionId, body: &RepositoryEventBody) {
    match body {
        RepositoryEventBody::NoteAttached { note_id, .. } => state.notes.push(Attributed {
            assertion_id,
            value: *note_id,
        }),
        RepositoryEventBody::Tagged { tag_id, .. } => state.tags.push(Attributed {
            assertion_id,
            value: *tag_id,
        }),
        RepositoryEventBody::Untagged { tag_id, .. } => state.tags.retain(|t| t.value != *tag_id),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::address::Address;
    use crate::enums::RepositoryType;
    use crate::ids::{AgentId, AssertionId, HumanId, RepositoryId};
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use crate::repository::command::RepositoryCommand;
    use crate::repository::error::RepositoryError;
    use crate::repository::event::RepositoryEventBody;
    use crate::repository::state::RepositoryState;
    use time::macros::datetime;
    use uuid::Uuid;

    fn repo(n: u128) -> RepositoryId {
        RepositoryId::from_uuid(Uuid::from_u128(n))
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

    fn apply_all(state: &mut RepositoryState, events: &[crate::repository::event::RepositoryEvent]) {
        for event in events {
            evolve(state, event);
        }
    }

    fn created_repository(id: u128) -> RepositoryState {
        let mut state = RepositoryState::default();
        let events = decide(
            &state,
            RepositoryCommand::CreateRepository {
                repository_id: repo(id),
                human_id: HumanId::new("R1"),
            },
            &meta(1),
        )
        .unwrap();
        apply_all(&mut state, &events);
        state
    }

    #[test]
    fn recreating_an_existing_repository_is_rejected() {
        let state = created_repository(1);
        let err = decide(
            &state,
            RepositoryCommand::CreateRepository {
                repository_id: repo(1),
                human_id: HumanId::new("R1"),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, RepositoryError::AlreadyExists(repo(1)));
    }

    #[test]
    fn command_against_absent_repository_is_not_found() {
        let state = RepositoryState::default();
        let err = decide(
            &state,
            RepositoryCommand::SetName {
                repository_id: repo(7),
                name: "Riksarkivet".to_owned(),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, RepositoryError::NotFound(repo(7)));
    }

    #[test]
    fn setting_an_empty_name_is_rejected() {
        let state = created_repository(1);
        let err = decide(
            &state,
            RepositoryCommand::SetName {
                repository_id: repo(1),
                name: "  ".to_owned(),
            },
            &meta(2),
        )
        .unwrap_err();
        assert_eq!(err, RepositoryError::EmptyName);
    }

    #[test]
    fn name_is_last_writer_wins_and_addresses_accumulate() {
        let mut state = created_repository(1);
        for command in [
            RepositoryCommand::SetName {
                repository_id: repo(1),
                name: "Statsarkivet".to_owned(),
            },
            RepositoryCommand::SetRepositoryType {
                repository_id: repo(1),
                repository_type: RepositoryType::Archive,
            },
            RepositoryCommand::AddAddress {
                repository_id: repo(1),
                address: Address {
                    locality: Some("Bergen".to_owned()),
                    ..Address::default()
                },
            },
        ] {
            let events = decide(&state, command, &meta(2)).unwrap();
            apply_all(&mut state, &events);
        }
        assert_eq!(state.name.as_ref().map(|n| n.value.as_str()), Some("Statsarkivet"));
        assert_eq!(
            state.repository_type.as_ref().map(|t| &t.value),
            Some(&RepositoryType::Archive)
        );
        assert_eq!(state.addresses.len(), 1);
    }

    #[test]
    fn retracting_a_name_removes_it_non_destructively() {
        let mut state = created_repository(1);
        let name = decide(
            &state,
            RepositoryCommand::SetName {
                repository_id: repo(1),
                name: "Statsarkivet".to_owned(),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &name);
        let target = AssertionId::from_uuid(Uuid::from_u128(2));
        assert!(state.name.is_some());

        let retract = decide(
            &state,
            RepositoryCommand::RetractAssertion {
                repository_id: repo(1),
                target,
            },
            &meta(3),
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.name.is_none());
        assert!(!state.live_assertions.contains(&target));
    }

    #[test]
    fn superseding_a_name_emits_supersession_then_replacement() {
        let mut state = created_repository(1);
        let first = decide(
            &state,
            RepositoryCommand::SetName {
                repository_id: repo(1),
                name: "Statsarkivet".to_owned(),
            },
            &meta(2),
        )
        .unwrap();
        apply_all(&mut state, &first);
        let target = AssertionId::from_uuid(Uuid::from_u128(2));

        let events = decide(
            &state,
            RepositoryCommand::SupersedeAssertion {
                repository_id: repo(1),
                target,
                replacement: Box::new(RepositoryCommand::SetName {
                    repository_id: repo(1),
                    name: "Riksarkivet".to_owned(),
                }),
            },
            &meta(3),
        )
        .unwrap();
        assert_eq!(events.len(), 2);
        assert!(matches!(
            events[0].body,
            RepositoryEventBody::AssertionSuperseded { .. }
        ));
        apply_all(&mut state, &events);
        assert_eq!(state.name.as_ref().map(|n| n.value.as_str()), Some("Riksarkivet"));
    }

    #[test]
    fn attached_notes_and_tags_project_and_retract_clears_them() {
        use crate::ids::{NoteId, TagId};

        let mut state = created_repository(1);
        let note = NoteId::from_uuid(Uuid::from_u128(0x40));
        let tag = TagId::from_uuid(Uuid::from_u128(0x41));
        for (assertion, command) in [
            (
                2,
                RepositoryCommand::AttachNote {
                    repository_id: repo(1),
                    note_id: note,
                },
            ),
            (
                3,
                RepositoryCommand::Tag {
                    repository_id: repo(1),
                    tag_id: tag,
                },
            ),
        ] {
            let events = decide(&state, command, &meta(assertion)).unwrap();
            apply_all(&mut state, &events);
        }
        assert_eq!(state.notes.len(), 1);
        assert_eq!(state.tags.len(), 1);

        let note_assertion = AssertionId::from_uuid(Uuid::from_u128(2));
        let retract = decide(
            &state,
            RepositoryCommand::RetractAssertion {
                repository_id: repo(1),
                target: note_assertion,
            },
            &meta(4),
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.notes.is_empty(), "the retracted note is gone");
        assert_eq!(state.tags.len(), 1, "the tag is untouched");
    }
}
