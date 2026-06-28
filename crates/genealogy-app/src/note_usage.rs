//! The reverse note index (Phase 5 PR 10): given a note, which records reference it.
//!
//! Note › References renders every record a note informs ("John Smith", "Marriage 1876"). Records
//! attach notes, but no aggregate stores the inverse, so this scans every note-bearing projection
//! once (person, family, event, place, source, citation, repository, `dna_test`, `dna_match`) and
//! inverts the attachments to a `NoteId -> [UsingRecordRef]` map. `DnaTest`/`DnaMatch` project notes since
//! Phase 5 PR 11, so a note attached to a DNA record is surfaced here too. The join lives in the
//! app/db layer (the cross-aggregate-joins dependency note).

use std::collections::HashMap;

use genealogy_core::ids::NoteId;

use crate::dto::{UsingKind, UsingRecordRef};
use crate::error::AppError;
use crate::person::list_persons;
use crate::workspace::Workspace;

/// A `NoteId -> [UsingRecordRef]` reverse index over the note-bearing aggregates.
pub(crate) struct NoteUsage {
    by_note: HashMap<NoteId, Vec<UsingRecordRef>>,
}

impl NoteUsage {
    /// Scans every note-bearing projection and inverts the attachments.
    pub(crate) async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let person_names: HashMap<String, String> = list_persons(workspace)
            .await?
            .into_iter()
            .filter_map(|p| p.display_name.map(|name| (p.human_id, name)))
            .collect();

        let mut by_note: HashMap<NoteId, Vec<UsingRecordRef>> = HashMap::new();
        scan_persons(workspace, &person_names, &mut by_note).await?;
        scan_families(workspace, &mut by_note).await?;
        scan_events(workspace, &mut by_note).await?;
        scan_places(workspace, &mut by_note).await?;
        scan_sources(workspace, &mut by_note).await?;
        scan_citations(workspace, &mut by_note).await?;
        scan_repositories(workspace, &mut by_note).await?;
        scan_dna_tests(workspace, &mut by_note).await?;
        scan_dna_matches(workspace, &mut by_note).await?;
        Ok(Self { by_note })
    }

    /// The records that reference `note`, in scan order (empty if none).
    pub(crate) fn used_by(&self, note: NoteId) -> Vec<UsingRecordRef> {
        self.by_note.get(&note).cloned().unwrap_or_default()
    }
}

/// Pushes one referencing record onto a note's bucket.
fn push(map: &mut HashMap<NoteId, Vec<UsingRecordRef>>, note: NoteId, record: UsingRecordRef) {
    map.entry(note).or_default().push(record);
}

/// Inverts person note attachments.
async fn scan_persons(
    workspace: &Workspace,
    person_names: &HashMap<String, String>,
    map: &mut HashMap<NoteId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_persons().await? {
        let (Some(id), Some(human_id)) = (view.person_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = person_names.get(&human_id).cloned();
        for note in view.notes() {
            push(
                map,
                note,
                UsingRecordRef {
                    kind: UsingKind::Person,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts family note attachments.
async fn scan_families(workspace: &Workspace, map: &mut HashMap<NoteId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_families().await? {
        let (Some(id), Some(human_id)) = (view.family_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        for note in view.notes() {
            push(
                map,
                note,
                UsingRecordRef {
                    kind: UsingKind::Family,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: None,
                },
            );
        }
    }
    Ok(())
}

/// Inverts event note attachments.
async fn scan_events(workspace: &Workspace, map: &mut HashMap<NoteId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_events().await? {
        let (Some(id), Some(human_id)) = (view.event_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.description().map(ToOwned::to_owned);
        for note in view.notes() {
            push(
                map,
                note,
                UsingRecordRef {
                    kind: UsingKind::Event,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts place note attachments.
async fn scan_places(workspace: &Workspace, map: &mut HashMap<NoteId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_places().await? {
        let (Some(id), Some(human_id)) = (view.place_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.names().first().map(|n| n.text.clone());
        for note in view.notes() {
            push(
                map,
                note,
                UsingRecordRef {
                    kind: UsingKind::Place,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts source note attachments.
async fn scan_sources(workspace: &Workspace, map: &mut HashMap<NoteId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_sources().await? {
        let (Some(id), Some(human_id)) = (view.source_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.title().map(ToOwned::to_owned);
        for note in view.notes() {
            push(
                map,
                note,
                UsingRecordRef {
                    kind: UsingKind::Source,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts citation note attachments.
async fn scan_citations(workspace: &Workspace, map: &mut HashMap<NoteId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_citations().await? {
        let (Some(id), Some(human_id)) = (view.citation_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        for note in view.notes() {
            push(
                map,
                note,
                UsingRecordRef {
                    kind: UsingKind::Citation,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: None,
                },
            );
        }
    }
    Ok(())
}

/// Inverts repository note attachments.
async fn scan_repositories(
    workspace: &Workspace,
    map: &mut HashMap<NoteId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_repositories().await? {
        let (Some(id), Some(human_id)) = (view.repository_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.name().map(ToOwned::to_owned);
        for note in view.notes() {
            push(
                map,
                note,
                UsingRecordRef {
                    kind: UsingKind::Repository,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}

/// Inverts DNA-test note attachments (notes projected since Phase 5 PR 11).
async fn scan_dna_tests(workspace: &Workspace, map: &mut HashMap<NoteId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_dna_tests().await? {
        let (Some(id), Some(human_id)) = (view.dna_test_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        for note in view.notes() {
            push(
                map,
                note,
                UsingRecordRef {
                    kind: UsingKind::DnaTest,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: None,
                },
            );
        }
    }
    Ok(())
}

/// Inverts DNA-match note attachments (notes projected since Phase 5 PR 11).
async fn scan_dna_matches(
    workspace: &Workspace,
    map: &mut HashMap<NoteId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_dna_matches().await? {
        let (Some(id), Some(human_id)) = (view.dna_match_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.predicted_relationship().map(ToOwned::to_owned);
        for note in view.notes() {
            push(
                map,
                note,
                UsingRecordRef {
                    kind: UsingKind::DnaMatch,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: label.clone(),
                },
            );
        }
    }
    Ok(())
}
