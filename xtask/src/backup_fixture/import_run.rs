//! Import-run fixture events: every `ImportRunEventBody` variant, and a note whose assertions carry
//! the finished run's record origin (ADR 0037), so a restore proves origins survive a backup.

use vitni_core::ids::{HumanId, ImportRunId, NoteId};
use vitni_core::import_run::{
    AbandonReason, ImportCounts, ImportRunEvent, ImportRunEventBody, ImportRunState, ResolutionDecision,
};
use vitni_core::note::{NoteEvent, NoteEventBody, NoteState};
use vitni_core::origin::{DatasetId, RecordOrigin};
use vitni_core::text::{MediaType, RichText};

use crate::backup_fixture::Builder;

/// Pushes a finished run that imported a note and resolved the first person, then an abandoned run.
pub(crate) fn events(builder: &mut Builder) {
    let dataset = DatasetId::lineage("gedcom", builder.uuid());
    let finished = ImportRunId::from_uuid(builder.uuid());
    push_run(builder, finished, &dataset, "hansen-tree.ged");
    push_imported_note(builder, finished, &dataset);
    let mut bodies = Vec::new();
    if let Some(person) = builder.ids.persons.first() {
        bodies.push(ImportRunEventBody::ItemResolved {
            run_id: finished,
            dataset: dataset.clone(),
            record: "I1".to_owned(),
            item: None,
            kind: "person".to_owned(),
            aggregate_id: person.as_uuid(),
            decision: ResolutionDecision::ExternalId,
        });
    }
    let mut counts = ImportCounts {
        resolved: 1,
        commands: 2,
        records: Some(2),
        ..ImportCounts::default()
    };
    counts.created.insert("note".to_owned(), 1);
    bodies.push(ImportRunEventBody::ImportRunFinished {
        run_id: finished,
        counts,
    });
    for body in bodies {
        let meta = builder.meta();
        builder.push::<ImportRunState>(finished, ImportRunEvent::new(&meta, body));
    }

    let abandoned = ImportRunId::from_uuid(builder.uuid());
    push_run(builder, abandoned, &dataset, "hansen-tree-2026.ged");
    let meta = builder.meta();
    builder.push::<ImportRunState>(
        abandoned,
        ImportRunEvent::new(
            &meta,
            ImportRunEventBody::ImportRunAbandoned {
                run_id: abandoned,
                reason: AbandonReason::Cancelled,
                counts: ImportCounts::default(),
            },
        ),
    );
}

fn push_run(builder: &mut Builder, run_id: ImportRunId, dataset: &DatasetId, file: &str) {
    let meta = builder.meta();
    builder.push::<ImportRunState>(
        run_id,
        ImportRunEvent::new(
            &meta,
            ImportRunEventBody::ImportRunStarted {
                run_id,
                plugin: "gedcom-import".to_owned(),
                plugin_version: "0.1.0".to_owned(),
                dataset: dataset.clone(),
                dataset_label: "hansen-tree.ged".to_owned(),
                source_label: file.to_owned(),
                source_path: None,
                file_asserted_at: None,
                dataset_hint: Some("GRAMPS|hansen-tree.ged".to_owned()),
            },
        ),
    );
}

fn push_imported_note(builder: &mut Builder, run: ImportRunId, dataset: &DatasetId) {
    let note_id = NoteId::from_uuid(builder.uuid());
    let bodies = [
        NoteEventBody::NoteCreated {
            note_id,
            human_id: HumanId::new("N0002"),
        },
        NoteEventBody::RichTextSet {
            note_id,
            text: RichText {
                text: "Emigrated from Bergen in 1869.".to_owned(),
                media_type: MediaType::Markdown,
                language: None,
                translator: None,
                translations: Vec::new(),
            },
        },
    ];
    for body in bodies {
        let mut meta = builder.meta();
        meta.context.origin = Some(Box::new(RecordOrigin {
            dataset: dataset.clone(),
            record: "N1".to_owned(),
            item: None,
            digest: None,
            run,
        }));
        builder.push::<NoteState>(note_id, NoteEvent::new(&meta, body));
    }
}
