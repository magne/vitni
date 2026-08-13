//! The pure Citation decision core (ADR 0004 §3) and the `evolve` fold.
//!
//! `decide(state, command, meta, refs) -> Result<Vec<CitationEvent>, CitationError>` reads no clock
//! and generates no id, and does not read another aggregate's projection itself: the
//! cross-aggregate facts arrive in `refs`, resolved by the `Services`-backed adapter
//! ([`crate::citation::aggregate`]) from the [`CitationRefResolver`](super::ref_resolver). So the
//! rule (`UnknownSource`) lives here, in the pure core, while the impure read stays at the edge.

use crate::assertions::Attributed;
use crate::citation::command::CitationCommand;
use crate::citation::error::CitationError;
use crate::citation::event::{CitationEvent, CitationEventBody};
use crate::citation::ref_resolver::CitationRefs;
use crate::citation::state::{CitationState, CreationStamp};
use crate::ids::CitationId;
use crate::provenance::AssertionMeta;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// # Errors
///
/// Returns a [`CitationError`] when the command violates an invariant: creating a citation that
/// exists, a command against an absent citation, creating a citation against a source the projection
/// does not know (`refs.source_exists == false`, the §9 aggregate-tax check), or correcting an
/// unknown assertion.
pub fn decide(
    state: &CitationState,
    command: CitationCommand,
    meta: &AssertionMeta,
    refs: &CitationRefs,
) -> Result<Vec<CitationEvent>, CitationError> {
    match command {
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
            Ok(one(
                meta,
                CitationEventBody::CitationCreated {
                    citation_id,
                    human_id,
                    source_id,
                },
            ))
        }
        // The plain commands share the same shape — exist-check then emit one event — so they
        // delegate to `simple_body` (exhaustive over them). Only `citation_id` is bound here (it is
        // `Copy`), leaving `command` intact to hand over.
        CitationCommand::SetPage { citation_id, .. }
        | CitationCommand::AssertDate { citation_id, .. }
        | CitationCommand::SetConfidence { citation_id, .. }
        | CitationCommand::SetEvidenceAnalysis { citation_id, .. }
        | CitationCommand::AddAttribute { citation_id, .. }
        | CitationCommand::AttachMedia { citation_id, .. }
        | CitationCommand::AttachNote { citation_id, .. }
        | CitationCommand::Tag { citation_id, .. }
        | CitationCommand::Untag { citation_id, .. }
        | CitationCommand::SetRestrictions { citation_id, .. } => {
            ensure_exists(state, citation_id)?;
            Ok(one(meta, simple_body(command)))
        }
        CitationCommand::SetHumanId { citation_id, human_id } => {
            ensure_exists(state, citation_id)?;
            let old_human_id = state.human_id.clone().unwrap_or_else(|| human_id.clone());
            Ok(one(
                meta,
                CitationEventBody::HumanIdChanged {
                    citation_id,
                    human_id,
                    old_human_id,
                },
            ))
        }
        CitationCommand::RetractAssertion { citation_id, target } => {
            ensure_exists(state, citation_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(CitationError::RetractsMissingAssertion(target));
            }
            Ok(one(meta, CitationEventBody::AssertionRetracted { citation_id, target }))
        }
        CitationCommand::SupersedeAssertion {
            citation_id,
            target,
            replacement,
        } => {
            ensure_exists(state, citation_id)?;
            if !state.live_assertions.contains(&target) {
                return Err(CitationError::SupersedesMissingAssertion(target));
            }
            let mut events = one(meta, CitationEventBody::AssertionSuperseded { citation_id, target });
            events.extend(decide(state, *replacement, meta, refs)?);
            Ok(events)
        }
    }
}

/// Maps a plain command to its event body (the existence check is done by `decide`).
///
/// Exhaustive over the plain commands; the lifecycle/cross-aggregate commands never reach here.
fn simple_body(command: CitationCommand) -> CitationEventBody {
    match command {
        CitationCommand::SetPage { citation_id, page } => CitationEventBody::PageSet { citation_id, page },
        CitationCommand::AssertDate { citation_id, date } => CitationEventBody::DateAsserted { citation_id, date },
        CitationCommand::SetConfidence {
            citation_id,
            confidence,
        } => CitationEventBody::ConfidenceSet {
            citation_id,
            confidence,
        },
        CitationCommand::SetEvidenceAnalysis { citation_id, analysis } => {
            CitationEventBody::EvidenceAnalysisSet { citation_id, analysis }
        }
        CitationCommand::AddAttribute { citation_id, attribute } => {
            CitationEventBody::AttributeAdded { citation_id, attribute }
        }
        CitationCommand::AttachMedia { citation_id, media } => CitationEventBody::MediaAttached { citation_id, media },
        CitationCommand::AttachNote { citation_id, note_id } => {
            CitationEventBody::NoteAttached { citation_id, note_id }
        }
        CitationCommand::Tag { citation_id, tag_id } => CitationEventBody::Tagged { citation_id, tag_id },
        CitationCommand::Untag { citation_id, tag_id } => CitationEventBody::Untagged { citation_id, tag_id },
        CitationCommand::SetRestrictions {
            citation_id,
            restrictions,
        } => CitationEventBody::RestrictionsChanged {
            citation_id,
            restrictions,
        },
        CitationCommand::CreateCitation { .. }
        | CitationCommand::RetractAssertion { .. }
        | CitationCommand::SupersedeAssertion { .. }
        | CitationCommand::SetHumanId { .. } => unreachable!("handled by decide"),
    }
}

/// Builds the single-event vector for a body stamped with `meta`.
fn one(meta: &AssertionMeta, body: CitationEventBody) -> Vec<CitationEvent> {
    vec![CitationEvent::new(meta, body)]
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
    let assertion_id = event.assertion_id;
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
            state.created = Some(CreationStamp {
                by: event.context.operator.clone(),
                at: event.context.occurred_at,
            });
            state.live_assertions.insert(assertion_id);
        }
        CitationEventBody::PageSet { page, .. } => {
            state.page = Some(Attributed {
                assertion_id,
                value: page.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        CitationEventBody::DateAsserted { date, .. } => {
            state.date = Some(Attributed {
                assertion_id,
                value: date.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        CitationEventBody::ConfidenceSet { confidence, .. } => {
            state.confidence = Some(Attributed {
                assertion_id,
                value: *confidence,
            });
            state.live_assertions.insert(assertion_id);
        }
        CitationEventBody::EvidenceAnalysisSet { analysis, .. } => {
            state.evidence_analysis = Some(Attributed {
                assertion_id,
                value: *analysis,
            });
            state.live_assertions.insert(assertion_id);
        }
        CitationEventBody::AttributeAdded { attribute, .. } => {
            state.attributes.push(Attributed {
                assertion_id,
                value: attribute.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        CitationEventBody::MediaAttached { media, .. } => {
            state.media.push(Attributed {
                assertion_id,
                value: media.clone(),
            });
            state.live_assertions.insert(assertion_id);
        }
        CitationEventBody::NoteAttached { note_id, .. } => {
            state.notes.push(Attributed {
                assertion_id,
                value: *note_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        CitationEventBody::Tagged { tag_id, .. } => {
            state.tags.push(Attributed {
                assertion_id,
                value: *tag_id,
            });
            state.live_assertions.insert(assertion_id);
        }
        CitationEventBody::Untagged { tag_id, .. } => {
            state.tags.retain(|t| t.value != *tag_id);
            state.live_assertions.insert(assertion_id);
        }
        CitationEventBody::RestrictionsChanged { restrictions, .. } => {
            state.restrictions.clone_from(restrictions);
            state.restrictions_assertion = Some(assertion_id);
            state.live_assertions.insert(assertion_id);
        }
        CitationEventBody::HumanIdChanged { human_id, .. } => {
            state.human_id = Some(human_id.clone());
        }
        CitationEventBody::AssertionRetracted { target, .. }
        | CitationEventBody::AssertionSuperseded { target, .. } => {
            state.remove_assertion(*target);
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
                confidence: Some(Confidence::Normal),
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
    fn citation_created_records_the_creating_operators_provenance() {
        // The fold retains who created the citation, and when, for the UI's "asserted by" line.
        let mut state = CitationState::default();
        let mut meta = meta(1);
        meta.context.operator.display = Some("magne".to_owned());
        let events = decide(
            &state,
            CitationCommand::CreateCitation {
                citation_id: citation(1),
                human_id: HumanId::new("C1"),
                source_id: source(1),
            },
            &meta,
            &SOURCE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &events);
        let created = state.created.as_ref().expect("creation stamp is folded");
        assert_eq!(created.by.display.as_deref(), Some("magne"));
        assert_eq!(created.at, Timestamp::new(datetime!(2026-06-19 12:00:00 UTC)));
    }

    #[test]
    fn citation_created_by_software_preserves_the_agent_kind() {
        // A display-less software agent: the typed stamp keeps the Human/Software distinction the
        // old stringly `created_by` erased (finding 7, ADR 0021 §4).
        let mut state = CitationState::default();
        let mut meta = meta(1);
        meta.context.operator.display = None;
        meta.context.operator.kind = AgentKind::Software {
            name: "vitni-import".to_owned(),
            version: "1.0".to_owned(),
        };
        let events = decide(
            &state,
            CitationCommand::CreateCitation {
                citation_id: citation(1),
                human_id: HumanId::new("C1"),
                source_id: source(1),
            },
            &meta,
            &SOURCE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &events);
        let created = state.created.as_ref().expect("creation stamp is folded");
        assert_eq!(
            created.by.kind,
            AgentKind::Software {
                name: "vitni-import".to_owned(),
                version: "1.0".to_owned(),
            }
        );
        assert_eq!(created.by.display, None);
    }

    #[test]
    fn retracting_a_restriction_change_clears_the_restrictions() {
        use crate::enums::Restriction;
        use std::collections::BTreeSet;

        let mut state = created_citation(1);
        let set = decide(
            &state,
            CitationCommand::SetRestrictions {
                citation_id: citation(1),
                restrictions: BTreeSet::from([Restriction::Locked]),
            },
            &meta(2),
            &SOURCE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &set);
        assert_eq!(state.restrictions, BTreeSet::from([Restriction::Locked]));

        let retract = decide(
            &state,
            CitationCommand::RetractAssertion {
                citation_id: citation(1),
                target: AssertionId::from_uuid(Uuid::from_u128(2)),
            },
            &meta(3),
            &SOURCE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &retract);
        assert!(state.restrictions.is_empty(), "retracting the change clears the set");
        assert_eq!(state.restrictions_assertion, None);
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
        assert_eq!(state.page.as_ref().map(|p| p.value.as_str()), Some("p. 42"));
    }

    #[test]
    fn retracting_an_unknown_assertion_is_rejected() {
        let state = created_citation(1);
        let unknown = AssertionId::from_uuid(Uuid::from_u128(999));
        let err = decide(
            &state,
            CitationCommand::RetractAssertion {
                citation_id: citation(1),
                target: unknown,
            },
            &meta(2),
            &SOURCE_PRESENT,
        )
        .unwrap_err();
        assert_eq!(err, CitationError::RetractsMissingAssertion(unknown));
    }

    #[test]
    fn retracting_a_live_page_removes_it_non_destructively() {
        let mut state = created_citation(1);
        let page_events = decide(
            &state,
            CitationCommand::SetPage {
                citation_id: citation(1),
                page: "p. 1".to_owned(),
            },
            &meta(2),
            &SOURCE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &page_events);
        let page_assertion = AssertionId::from_uuid(Uuid::from_u128(2));
        assert!(state.page.is_some());

        let retract = decide(
            &state,
            CitationCommand::RetractAssertion {
                citation_id: citation(1),
                target: page_assertion,
            },
            &meta(3),
            &SOURCE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &retract);

        assert!(state.page.is_none());
        assert!(!state.live_assertions.contains(&page_assertion));
    }

    #[test]
    fn superseding_emits_a_supersession_then_the_replacement_event() {
        let mut state = created_citation(1);
        let first = decide(
            &state,
            CitationCommand::SetPage {
                citation_id: citation(1),
                page: "p. 1".to_owned(),
            },
            &meta(2),
            &SOURCE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &first);
        let target = AssertionId::from_uuid(Uuid::from_u128(2));

        let events = decide(
            &state,
            CitationCommand::SupersedeAssertion {
                citation_id: citation(1),
                target,
                replacement: Box::new(CitationCommand::SetPage {
                    citation_id: citation(1),
                    page: "p. 42".to_owned(),
                }),
            },
            &meta(3),
            &SOURCE_PRESENT,
        )
        .unwrap();

        assert_eq!(events.len(), 2);
        assert!(matches!(events[0].body, CitationEventBody::AssertionSuperseded { .. }));
        assert!(matches!(events[1].body, CitationEventBody::PageSet { .. }));

        apply_all(&mut state, &events);
        assert_eq!(state.page.as_ref().map(|p| p.value.as_str()), Some("p. 42"));
        assert!(!state.live_assertions.contains(&target));
    }

    #[test]
    fn confidence_is_last_writer_wins_and_attributes_accumulate() {
        use crate::provenance::Confidence;
        use crate::text::Attribute;

        let mut state = created_citation(1);
        for (assertion, command) in [
            (
                2,
                CitationCommand::SetConfidence {
                    citation_id: citation(1),
                    confidence: Confidence::Low,
                },
            ),
            (
                3,
                CitationCommand::SetConfidence {
                    citation_id: citation(1),
                    confidence: Confidence::High,
                },
            ),
            (
                4,
                CitationCommand::AddAttribute {
                    citation_id: citation(1),
                    attribute: Attribute {
                        attribute_type: "quality".to_owned(),
                        value: "good".to_owned(),
                    },
                },
            ),
        ] {
            let events = decide(&state, command, &meta(assertion), &SOURCE_PRESENT).unwrap();
            apply_all(&mut state, &events);
        }
        assert_eq!(state.confidence.as_ref().map(|c| c.value), Some(Confidence::High));
        assert_eq!(state.attributes.len(), 1);
    }

    #[test]
    fn attachments_and_tags_project_into_state() {
        use crate::ids::{MediaId, NoteId, TagId};
        use crate::text::MediaRef;

        let note = NoteId::from_uuid(Uuid::from_u128(0x10));
        let tag = TagId::from_uuid(Uuid::from_u128(0x20));
        let media = MediaRef {
            media_id: MediaId::from_uuid(Uuid::from_u128(0x30)),
            crop: None,
            caption: None,
            citations: Vec::new(),
        };
        let mut state = created_citation(1);
        for (assertion, command) in [
            (
                2,
                CitationCommand::AttachMedia {
                    citation_id: citation(1),
                    media: media.clone(),
                },
            ),
            (
                3,
                CitationCommand::AttachNote {
                    citation_id: citation(1),
                    note_id: note,
                },
            ),
            (
                4,
                CitationCommand::Tag {
                    citation_id: citation(1),
                    tag_id: tag,
                },
            ),
        ] {
            let events = decide(&state, command, &meta(assertion), &SOURCE_PRESENT).unwrap();
            apply_all(&mut state, &events);
        }
        assert_eq!(state.media.len(), 1);
        assert_eq!(state.notes.len(), 1);
        assert_eq!(state.tags.len(), 1);

        // Untag removes the tag; retracting the media assertion removes the media non-destructively.
        let untag = decide(
            &state,
            CitationCommand::Untag {
                citation_id: citation(1),
                tag_id: tag,
            },
            &meta(5),
            &SOURCE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &untag);
        let media_assertion = AssertionId::from_uuid(Uuid::from_u128(2));
        let retract = decide(
            &state,
            CitationCommand::RetractAssertion {
                citation_id: citation(1),
                target: media_assertion,
            },
            &meta(6),
            &SOURCE_PRESENT,
        )
        .unwrap();
        apply_all(&mut state, &retract);

        assert!(state.tags.is_empty(), "untag clears the tag");
        assert!(state.media.is_empty(), "retract removes the attached media");
        assert_eq!(state.notes.len(), 1, "the note remains");
    }
}
