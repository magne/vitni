//! The pure Source decision core (ADR 0004 §3) and the `evolve` fold.
//!
//! `decide(state, command, meta, refs)` reads no clock, generates no id, and reads no other
//! aggregate's projection itself: the cross-aggregate fact (does the linked repository exist?)
//! arrives in `refs`, resolved before `decide` by the `Services`-backed adapter from the
//! [`SourceRefResolver`](super::ref_resolver) — mirroring Citation→Source.

use crate::assertions::{Asserted, Attributed};
use crate::ids::{AssertionId, SourceId};
use crate::provenance::AssertionMeta;
use crate::source::command::SourceCommand;
use crate::source::error::SourceError;
use crate::source::event::{SourceEvent, SourceEventBody};
use crate::source::ref_resolver::SourceRefs;
use crate::source::state::SourceState;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`SourceError`] when the command violates an invariant: creating a source that exists,
/// a command against an absent source, linking a repository the projection does not know
/// (`refs.repository_exists == false`, the §9 aggregate-tax check), or correcting an unknown
/// assertion.
pub fn decide(
    state: &SourceState,
    command: SourceCommand,
    meta: &AssertionMeta,
    refs: &SourceRefs,
) -> Result<Vec<SourceEvent>, SourceError> {
    match command {
        SourceCommand::CreateSource { source_id, human_id } => {
            if state.exists {
                return Err(SourceError::AlreadyExists(source_id));
            }
            Ok(one(meta, SourceEventBody::SourceCreated { source_id, human_id }))
        }
        SourceCommand::LinkRepository { source_id, repo_ref } => {
            ensure_exists(state, source_id)?;
            if !refs.repository_exists {
                return Err(SourceError::UnknownRepository(repo_ref.repository_id));
            }
            Ok(one(meta, SourceEventBody::RepositoryLinked { source_id, repo_ref }))
        }
        // The single-fact setters all share the same shape — exist-check then emit one event — so
        // they delegate to `setter_body` (exhaustive over them). Only `source_id` is bound here (it
        // is `Copy`), leaving `command` intact to hand over.
        SourceCommand::SetTitle { source_id, .. }
        | SourceCommand::SetAuthor { source_id, .. }
        | SourceCommand::SetPubInfo { source_id, .. }
        | SourceCommand::SetAbbrev { source_id, .. }
        | SourceCommand::AddAttribute { source_id, .. }
        | SourceCommand::AttachMedia { source_id, .. }
        | SourceCommand::AttachNote { source_id, .. }
        | SourceCommand::Tag { source_id, .. }
        | SourceCommand::Untag { source_id, .. } => {
            ensure_exists(state, source_id)?;
            Ok(one(meta, setter_body(command)))
        }
        SourceCommand::SetRestrictions {
            source_id,
            restrictions,
        } => {
            ensure_exists(state, source_id)?;
            Ok(one(
                meta,
                SourceEventBody::RestrictionsChanged {
                    source_id,
                    restrictions,
                },
            ))
        }
        SourceCommand::SetHumanId { source_id, human_id } => {
            ensure_exists(state, source_id)?;
            let old_human_id = state.human_id.clone().unwrap_or_else(|| human_id.clone());
            Ok(one(
                meta,
                SourceEventBody::HumanIdChanged {
                    source_id,
                    human_id,
                    old_human_id,
                },
            ))
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
            events.extend(decide(state, *replacement, meta, refs)?);
            Ok(events)
        }
    }
}

/// Maps a single-fact setter command to its event body (the existence check is done by `decide`).
///
/// Exhaustive over the setter commands; the lifecycle/cross-aggregate commands never reach here.
fn setter_body(command: SourceCommand) -> SourceEventBody {
    match command {
        SourceCommand::SetTitle { source_id, title } => SourceEventBody::TitleSet { source_id, title },
        SourceCommand::SetAuthor { source_id, author } => SourceEventBody::AuthorSet { source_id, author },
        SourceCommand::SetPubInfo { source_id, pub_info } => SourceEventBody::PubInfoSet { source_id, pub_info },
        SourceCommand::SetAbbrev { source_id, abbrev } => SourceEventBody::AbbrevSet { source_id, abbrev },
        SourceCommand::AddAttribute { source_id, attribute } => {
            SourceEventBody::AttributeAdded { source_id, attribute }
        }
        SourceCommand::AttachMedia { source_id, media } => SourceEventBody::MediaAttached { source_id, media },
        SourceCommand::AttachNote { source_id, note_id } => SourceEventBody::NoteAttached { source_id, note_id },
        SourceCommand::Tag { source_id, tag_id } => SourceEventBody::Tagged { source_id, tag_id },
        SourceCommand::Untag { source_id, tag_id } => SourceEventBody::Untagged { source_id, tag_id },
        SourceCommand::CreateSource { .. }
        | SourceCommand::LinkRepository { .. }
        | SourceCommand::SetRestrictions { .. }
        | SourceCommand::RetractAssertion { .. }
        | SourceCommand::SupersedeAssertion { .. }
        | SourceCommand::SetHumanId { .. } => unreachable!("handled by decide"),
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
        SourceEventBody::AuthorSet { author, .. } => {
            state.author = Some(Attributed {
                assertion_id,
                value: author.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        SourceEventBody::PubInfoSet { pub_info, .. } => {
            state.pub_info = Some(Attributed {
                assertion_id,
                value: pub_info.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        SourceEventBody::AbbrevSet { abbrev, .. } => {
            state.abbrev = Some(Attributed {
                assertion_id,
                value: abbrev.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        SourceEventBody::RepositoryLinked { repo_ref, .. } => {
            state.repositories.push(Attributed {
                assertion_id,
                value: Asserted::from_context(repo_ref.clone(), &event.context),
            });
            state.live_assertions.insert(assertion_id);
        }
        SourceEventBody::AttributeAdded { attribute, .. } => {
            state.attributes.push(Attributed {
                assertion_id,
                value: attribute.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        SourceEventBody::MediaAttached { .. }
        | SourceEventBody::NoteAttached { .. }
        | SourceEventBody::Tagged { .. }
        | SourceEventBody::Untagged { .. } => {
            fold_attachment(state, assertion_id, &event.body);
            state.live_assertions.insert(assertion_id);
        }
        SourceEventBody::RestrictionsChanged { restrictions, .. } => {
            state.restrictions.clone_from(restrictions);
            state.restrictions_assertion = Some(assertion_id);
            state.live_assertions.insert(assertion_id);
        }
        SourceEventBody::HumanIdChanged { human_id, .. } => {
            state.human_id = Some(human_id.clone());
        }
        SourceEventBody::AssertionRetracted { target, .. } | SourceEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
        }
    }
}

/// Folds an attachment event (media/note/tag) into the projected state.
fn fold_attachment(state: &mut SourceState, assertion_id: AssertionId, body: &SourceEventBody) {
    match body {
        SourceEventBody::MediaAttached { media, .. } => state.media.push(Attributed {
            assertion_id,
            value: media.clone(),
        }),
        SourceEventBody::NoteAttached { note_id, .. } => state.notes.push(Attributed {
            assertion_id,
            value: *note_id,
        }),
        SourceEventBody::Tagged { tag_id, .. } => state.tags.push(Attributed {
            assertion_id,
            value: *tag_id,
        }),
        SourceEventBody::Untagged { tag_id, .. } => state.tags.retain(|t| t.value != *tag_id),
        _ => {}
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::enums::SourceMediaType;
    use crate::ids::{AgentId, AssertionId, HumanId, RepositoryId, SourceId};
    use crate::provenance::{Agent, AgentKind, AssertionMeta, Confidence, EventContext, Timestamp};
    use crate::repo_ref::RepoRef;
    use crate::source::command::SourceCommand;
    use crate::source::error::SourceError;
    use crate::source::event::SourceEventBody;
    use crate::source::ref_resolver::SourceRefs;
    use crate::source::state::SourceState;
    use time::macros::datetime;
    use uuid::Uuid;

    const REPO_PRESENT: SourceRefs = SourceRefs {
        repository_exists: true,
    };
    const REPO_MISSING: SourceRefs = SourceRefs {
        repository_exists: false,
    };

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
                confidence: Some(Confidence::Normal),
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
            &REPO_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &events);
        state
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
            &REPO_PRESENT,
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
            &REPO_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, SourceError::NotFound(source(7)));
    }

    #[test]
    fn title_and_author_are_last_writer_wins() {
        let mut state = created_source(1);
        for command in [
            SourceCommand::SetTitle {
                source_id: source(1),
                title: "Folketelling 1801".to_owned(),
            },
            SourceCommand::SetAuthor {
                source_id: source(1),
                author: "Statistisk sentralbyrå".to_owned(),
            },
        ] {
            let events = decide(&state, command, &meta(2), &REPO_PRESENT).unwrap();
            apply_all(&mut state, &events);
        }
        assert_eq!(
            state.title.as_ref().map(|t| t.value.as_str()),
            Some("Folketelling 1801")
        );
        assert_eq!(
            state.author.as_ref().map(|a| a.value.as_str()),
            Some("Statistisk sentralbyrå")
        );
    }

    #[test]
    fn linking_a_missing_repository_is_unknown_repository() {
        let state = created_source(1);
        let repo = RepositoryId::from_uuid(Uuid::from_u128(99));
        let err = decide(
            &state,
            SourceCommand::LinkRepository {
                source_id: source(1),
                repo_ref: RepoRef {
                    repository_id: repo,
                    call_number: None,
                    media_type: SourceMediaType::Film,
                },
            },
            &meta(2),
            &REPO_MISSING,
        )
        .unwrap_err();
        assert_eq!(err, SourceError::UnknownRepository(repo));
    }

    #[test]
    fn linking_a_present_repository_accumulates() {
        let mut state = created_source(1);
        let events = decide(
            &state,
            SourceCommand::LinkRepository {
                source_id: source(1),
                repo_ref: RepoRef {
                    repository_id: RepositoryId::from_uuid(Uuid::from_u128(2)),
                    call_number: Some("MS 1234".to_owned()),
                    media_type: SourceMediaType::Film,
                },
            },
            &meta(2),
            &REPO_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &events);
        assert_eq!(state.repositories.len(), 1);
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
            &REPO_PRESENT,
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
            &REPO_PRESENT,
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
            &REPO_PRESENT,
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
            &REPO_PRESENT,
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

    #[test]
    fn attached_notes_and_tags_project_and_retract_clears_them() {
        use crate::ids::{NoteId, TagId};

        let mut state = created_source(1);
        let note = NoteId::from_uuid(Uuid::from_u128(0x40));
        let tag = TagId::from_uuid(Uuid::from_u128(0x41));
        for (assertion, command) in [
            (
                2,
                SourceCommand::AttachNote {
                    source_id: source(1),
                    note_id: note,
                },
            ),
            (
                3,
                SourceCommand::Tag {
                    source_id: source(1),
                    tag_id: tag,
                },
            ),
        ] {
            let events = decide(&state, command, &meta(assertion), &REPO_PRESENT).unwrap();
            apply_all(&mut state, &events);
        }
        assert_eq!(state.notes.len(), 1);
        assert_eq!(state.tags.len(), 1);

        let note_assertion = AssertionId::from_uuid(Uuid::from_u128(2));
        let retract = decide(
            &state,
            SourceCommand::RetractAssertion {
                source_id: source(1),
                target: note_assertion,
            },
            &meta(4),
            &REPO_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.notes.is_empty(), "the retracted note is gone");
        assert_eq!(state.tags.len(), 1, "the tag is untouched");
    }

    #[test]
    fn untagging_removes_only_the_named_tag() {
        use crate::ids::TagId;

        let mut state = created_source(1);
        let tag = TagId::from_uuid(Uuid::from_u128(0x41));
        for (assertion, command) in [
            (
                2,
                SourceCommand::Tag {
                    source_id: source(1),
                    tag_id: tag,
                },
            ),
            (
                3,
                SourceCommand::Untag {
                    source_id: source(1),
                    tag_id: tag,
                },
            ),
        ] {
            let events = decide(&state, command, &meta(assertion), &REPO_PRESENT).unwrap();
            apply_all(&mut state, &events);
        }
        assert!(state.tags.is_empty());
    }

    #[test]
    fn a_linked_repository_carries_its_assertion_surety() {
        let mut state = created_source(1);
        let events = decide(
            &state,
            SourceCommand::LinkRepository {
                source_id: source(1),
                repo_ref: RepoRef {
                    repository_id: RepositoryId::from_uuid(Uuid::from_u128(2)),
                    call_number: Some("M432".to_owned()),
                    media_type: SourceMediaType::Film,
                },
            },
            &meta(2),
            &REPO_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &events);
        assert_eq!(state.repositories.len(), 1);
        assert_eq!(state.repositories[0].value.confidence, Some(Confidence::Normal));
    }

    #[test]
    fn retracting_a_restriction_change_clears_the_restrictions() {
        let mut state = created_source(1);
        let restrictions = std::collections::BTreeSet::from([crate::enums::Restriction::Locked]);
        let set = decide(
            &state,
            SourceCommand::SetRestrictions {
                source_id: source(1),
                restrictions: restrictions.clone(),
            },
            &meta(2),
            &REPO_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &set);
        assert_eq!(state.restrictions, restrictions);

        let retract = decide(
            &state,
            SourceCommand::RetractAssertion {
                source_id: source(1),
                target: crate::ids::AssertionId::from_uuid(uuid::Uuid::from_u128(2)),
            },
            &meta(3),
            &REPO_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.restrictions.is_empty(), "retracting the change clears the set");
        assert_eq!(state.restrictions_assertion, None);
    }
}
