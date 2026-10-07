//! The pure `ImportRun` decision core (ADR 0004 §3) and the `evolve` fold.

use crate::ids::ImportRunId;
use crate::import_run::command::{ImportRunCommand, NewImportRun};
use crate::import_run::error::ImportRunError;
use crate::import_run::event::{ImportRunEvent, ImportRunEventBody, ResolvedItem};
use crate::import_run::state::{ImportRunState, ImportRunStatus};
use crate::origin::DatasetId;
use crate::provenance::AssertionMeta;

/// Decides the events a command produces, or rejects it with a domain error.
///
/// Ending a run writes one `ItemResolved` per resolution followed by the terminal event, in one
/// commit: the resolutions are batched at the end rather than appended one by one, since each append
/// to a stream replays it.
///
/// # Errors
///
/// Returns an [`ImportRunError`] when a run is started twice, or a command targets a run that was
/// never started or has already ended.
pub fn decide(
    state: &ImportRunState,
    command: ImportRunCommand,
    meta: &AssertionMeta,
) -> Result<Vec<ImportRunEvent>, ImportRunError> {
    match command {
        ImportRunCommand::StartImportRun { run_id, run } => {
            if state.status != ImportRunStatus::NotStarted {
                return Err(ImportRunError::AlreadyExists(run_id));
            }
            Ok(vec![ImportRunEvent::new(meta, started(run_id, run))])
        }
        ImportRunCommand::FinishImportRun {
            run_id,
            resolved,
            counts,
        } => {
            let dataset = ensure_running(state, run_id)?;
            let mut events = resolutions(meta, run_id, dataset, resolved);
            events.push(ImportRunEvent::new(
                meta,
                ImportRunEventBody::ImportRunFinished { run_id, counts },
            ));
            Ok(events)
        }
        ImportRunCommand::AbandonImportRun {
            run_id,
            resolved,
            counts,
            reason,
        } => {
            let dataset = ensure_running(state, run_id)?;
            let mut events = resolutions(meta, run_id, dataset, resolved);
            events.push(ImportRunEvent::new(
                meta,
                ImportRunEventBody::ImportRunAbandoned { run_id, reason, counts },
            ));
            Ok(events)
        }
    }
}

fn started(run_id: ImportRunId, run: NewImportRun) -> ImportRunEventBody {
    let NewImportRun {
        plugin,
        plugin_version,
        dataset,
        dataset_label,
        source_label,
        source_path,
        file_asserted_at,
        dataset_hint,
    } = run;
    ImportRunEventBody::ImportRunStarted {
        run_id,
        plugin,
        plugin_version,
        dataset,
        dataset_label,
        source_label,
        source_path,
        file_asserted_at,
        dataset_hint,
    }
}

/// The run's dataset, or the reason a command cannot end the run.
fn ensure_running(state: &ImportRunState, run_id: ImportRunId) -> Result<&DatasetId, ImportRunError> {
    match (&state.status, &state.dataset) {
        (ImportRunStatus::Running, Some(dataset)) => Ok(dataset),
        (ImportRunStatus::NotStarted, _) | (ImportRunStatus::Running, None) => Err(ImportRunError::NotFound(run_id)),
        (ImportRunStatus::Finished | ImportRunStatus::Abandoned { .. }, _) => Err(ImportRunError::AlreadyEnded(run_id)),
    }
}

fn resolutions(
    meta: &AssertionMeta,
    run_id: ImportRunId,
    dataset: &DatasetId,
    resolved: Vec<ResolvedItem>,
) -> Vec<ImportRunEvent> {
    let mut events = Vec::with_capacity(resolved.len() + 1);
    for resolution in resolved {
        let ResolvedItem {
            record,
            item,
            kind,
            aggregate_id,
            decision,
        } = resolution;
        events.push(ImportRunEvent::new(
            meta,
            ImportRunEventBody::ItemResolved {
                run_id,
                dataset: dataset.clone(),
                record,
                item,
                kind,
                aggregate_id,
                decision,
            },
        ));
    }
    events
}

/// Applies an event to the state (the fold). No business logic lives here (ADR 0004 §3).
pub fn evolve(state: &mut ImportRunState, event: &ImportRunEvent) {
    match &event.body {
        ImportRunEventBody::ImportRunStarted {
            run_id,
            plugin,
            plugin_version,
            dataset,
            dataset_label,
            source_label,
            source_path,
            file_asserted_at,
            dataset_hint,
        } => {
            state.status = ImportRunStatus::Running;
            state.run_id = Some(*run_id);
            state.operator = Some(event.context.operator.clone());
            state.started_at = Some(event.context.occurred_at);
            state.plugin.clone_from(plugin);
            state.plugin_version.clone_from(plugin_version);
            state.dataset = Some(dataset.clone());
            state.dataset_label.clone_from(dataset_label);
            state.source_label.clone_from(source_label);
            state.source_path.clone_from(source_path);
            state.file_asserted_at = *file_asserted_at;
            state.dataset_hint.clone_from(dataset_hint);
        }
        ImportRunEventBody::ItemResolved { .. } => {}
        ImportRunEventBody::ImportRunFinished { counts, .. } => {
            state.status = ImportRunStatus::Finished;
            state.ended_at = Some(event.context.occurred_at);
            state.counts.clone_from(counts);
        }
        ImportRunEventBody::ImportRunAbandoned { reason, counts, .. } => {
            state.status = ImportRunStatus::Abandoned { reason: reason.clone() };
            state.ended_at = Some(event.context.occurred_at);
            state.counts.clone_from(counts);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{decide, evolve};
    use crate::ids::{AgentId, AssertionId, ImportRunId};
    use crate::import_run::command::{ImportRunCommand, NewImportRun};
    use crate::import_run::error::ImportRunError;
    use crate::import_run::event::{AbandonReason, ImportCounts, ImportRunEventBody, ResolutionDecision, ResolvedItem};
    use crate::import_run::state::{ImportRunState, ImportRunStatus};
    use crate::origin::DatasetId;
    use crate::provenance::{Agent, AgentKind, AssertionMeta, EventContext, Timestamp};
    use time::macros::datetime;
    use uuid::Uuid;

    fn run_id() -> ImportRunId {
        ImportRunId::from_uuid(Uuid::from_u128(7))
    }

    fn meta() -> AssertionMeta {
        AssertionMeta {
            assertion_id: AssertionId::from_uuid(Uuid::from_u128(1)),
            context: EventContext {
                operator: Agent {
                    kind: AgentKind::Human,
                    id: AgentId::from_uuid(Uuid::from_u128(0xA)),
                    display: Some("Ada".to_owned()),
                },
                occurred_at: Timestamp::new(datetime!(2026-09-28 12:00:00 UTC)),
                rationale: None,
                confidence: None,
                citations: Vec::new(),
                evidence_analysis: None,
                origin: None,
            },
        }
    }

    fn dataset() -> DatasetId {
        DatasetId::lineage("gedcom", Uuid::from_u128(3))
    }

    fn start() -> ImportRunCommand {
        ImportRunCommand::StartImportRun {
            run_id: run_id(),
            run: NewImportRun {
                plugin: "gedcom-import".to_owned(),
                plugin_version: "0.3.0".to_owned(),
                dataset: dataset(),
                dataset_label: "tree.ged".to_owned(),
                source_label: "tree.ged".to_owned(),
                source_path: Some("/home/ada/tree.ged".to_owned()),
                file_asserted_at: None,
                dataset_hint: Some("GRAMPS|tree.ged".to_owned()),
            },
        }
    }

    fn apply(state: &mut ImportRunState, command: ImportRunCommand) -> Result<usize, ImportRunError> {
        let events = decide(state, command, &meta())?;
        for event in &events {
            evolve(state, event);
        }
        Ok(events.len())
    }

    fn running() -> ImportRunState {
        let mut state = ImportRunState::default();
        apply(&mut state, start()).expect("start");
        state
    }

    fn resolved_person() -> ResolvedItem {
        ResolvedItem {
            record: "@I1@".to_owned(),
            item: None,
            kind: "person".to_owned(),
            aggregate_id: Uuid::from_u128(11),
            decision: ResolutionDecision::ExternalId,
        }
    }

    #[test]
    fn starting_records_who_ran_what_over_which_dataset() {
        let state = running();
        assert_eq!(state.status, ImportRunStatus::Running);
        assert_eq!(state.run_id, Some(run_id()));
        assert_eq!(state.dataset, Some(dataset()));
        assert_eq!(state.source_label, "tree.ged");
        assert_eq!(state.source_path.as_deref(), Some("/home/ada/tree.ged"));
        assert_eq!(state.dataset_hint.as_deref(), Some("GRAMPS|tree.ged"));
        assert_eq!(state.operator.and_then(|agent| agent.display).as_deref(), Some("Ada"));
    }

    #[test]
    fn a_run_started_before_dataset_hints_decodes_without_one() {
        let mut event = decide(&ImportRunState::default(), start(), &meta())
            .expect("start")
            .remove(0);
        let mut json = serde_json::to_value(&event.body).expect("encode");
        json.as_object_mut().expect("object").remove("dataset_hint");
        event.body = serde_json::from_value(json).expect("an event without the field decodes");
        let ImportRunEventBody::ImportRunStarted { dataset_hint, .. } = event.body else {
            panic!("not a start: {:?}", event.body);
        };
        assert_eq!(dataset_hint, None);
    }

    #[test]
    fn a_run_started_before_source_paths_decodes_without_one() {
        let mut event = decide(&ImportRunState::default(), start(), &meta())
            .expect("start")
            .remove(0);
        let mut json = serde_json::to_value(&event.body).expect("encode");
        json.as_object_mut().expect("object").remove("source_path");
        event.body = serde_json::from_value(json).expect("an event without the field decodes");
        let ImportRunEventBody::ImportRunStarted { source_path, .. } = event.body else {
            panic!("not a start: {:?}", event.body);
        };
        assert_eq!(source_path, None);
    }

    #[test]
    fn a_run_cannot_start_twice() {
        let mut state = running();
        assert_eq!(apply(&mut state, start()), Err(ImportRunError::AlreadyExists(run_id())));
    }

    #[test]
    fn finishing_writes_each_resolution_then_the_finish_in_one_decision() {
        let state = running();
        let counts = ImportCounts {
            resolved: 1,
            ..ImportCounts::default()
        };
        let events = decide(
            &state,
            ImportRunCommand::FinishImportRun {
                run_id: run_id(),
                resolved: vec![resolved_person()],
                counts: counts.clone(),
            },
            &meta(),
        )
        .expect("finish");
        assert_eq!(events.len(), 2);
        let ImportRunEventBody::ItemResolved {
            dataset: resolved_in,
            record,
            ..
        } = &events[0].body
        else {
            panic!("the resolution comes first, got {:?}", events[0].body);
        };
        assert_eq!((resolved_in, record.as_str()), (&dataset(), "@I1@"));
        assert_eq!(
            events[1].body,
            ImportRunEventBody::ImportRunFinished {
                run_id: run_id(),
                counts
            }
        );
    }

    #[test]
    fn an_abandoned_run_keeps_its_reason_and_counts() {
        let mut state = running();
        let counts = ImportCounts {
            commands: 4,
            ..ImportCounts::default()
        };
        apply(
            &mut state,
            ImportRunCommand::AbandonImportRun {
                run_id: run_id(),
                resolved: Vec::new(),
                counts: counts.clone(),
                reason: AbandonReason::Cancelled,
            },
        )
        .expect("abandon");
        assert_eq!(
            state.status,
            ImportRunStatus::Abandoned {
                reason: AbandonReason::Cancelled
            }
        );
        assert_eq!(state.counts, counts);
    }

    #[test]
    fn a_run_that_never_started_cannot_end() {
        let finish = ImportRunCommand::FinishImportRun {
            run_id: run_id(),
            resolved: Vec::new(),
            counts: ImportCounts::default(),
        };
        assert_eq!(
            decide(&ImportRunState::default(), finish, &meta()),
            Err(ImportRunError::NotFound(run_id()))
        );
    }

    #[test]
    fn an_ended_run_cannot_end_again() {
        let mut state = running();
        let finish = || ImportRunCommand::FinishImportRun {
            run_id: run_id(),
            resolved: Vec::new(),
            counts: ImportCounts::default(),
        };
        apply(&mut state, finish()).expect("first finish");
        assert_eq!(apply(&mut state, finish()), Err(ImportRunError::AlreadyEnded(run_id())));
        let abandon = ImportRunCommand::AbandonImportRun {
            run_id: run_id(),
            resolved: Vec::new(),
            counts: ImportCounts::default(),
            reason: AbandonReason::ResourceLimit,
        };
        assert_eq!(apply(&mut state, abandon), Err(ImportRunError::AlreadyEnded(run_id())));
    }

    #[test]
    fn created_total_sums_every_kind() {
        let mut counts = ImportCounts::default();
        counts.created.insert("person".to_owned(), 3);
        counts.created.insert("place".to_owned(), 2);
        assert_eq!(counts.created_total(), 5);
    }
}
