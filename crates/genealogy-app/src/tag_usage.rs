//! The reverse tag index (Phase 5 PR 11): given a tag, which records carry it, grouped by object
//! type with a count and a few examples (the Tag › Usage tab).
//!
//! Tags are applied on the *tagged* aggregate, not stored inversely on the Tag, so this scans every
//! tag-bearing projection once (person, family, event, place, source, citation, repository, media,
//! note, `dna_test`, `dna_match`) and inverts `view.tags()` to a `TagId -> [UsingRecordRef]` map. The
//! join lives in the app/db layer (the cross-aggregate-joins dependency note).

use std::collections::HashMap;

use genealogy_core::ids::TagId;

use crate::dto::{UsingKind, UsingRecordRef};
use crate::error::AppError;
use crate::person::list_persons;
use crate::workspace::Workspace;

/// How many examples to surface per object-type group on the Usage tab.
const MAX_EXAMPLES: usize = 3;

/// One object-type group on the Tag › Usage tab: the kind, how many records of that kind carry the
/// tag, and the first few as examples.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TagUsageGroup {
    /// The object type the rows belong to.
    pub kind: UsingKind,
    /// How many records of this kind carry the tag.
    pub count: usize,
    /// The first few carrying records, for the Examples column.
    pub examples: Vec<UsingRecordRef>,
}

/// A `TagId -> [UsingRecordRef]` reverse index over the tag-bearing aggregates.
pub(crate) struct TagUsage {
    by_tag: HashMap<TagId, Vec<UsingRecordRef>>,
}

impl TagUsage {
    /// Scans every tag-bearing projection and inverts the applications.
    pub(crate) async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let person_names: HashMap<String, String> = list_persons(workspace)
            .await?
            .into_iter()
            .filter_map(|p| p.display_name.map(|name| (p.human_id, name)))
            .collect();

        let mut by_tag: HashMap<TagId, Vec<UsingRecordRef>> = HashMap::new();
        scan_persons(workspace, &person_names, &mut by_tag).await?;
        scan_families(workspace, &mut by_tag).await?;
        scan_events(workspace, &mut by_tag).await?;
        scan_places(workspace, &mut by_tag).await?;
        scan_sources(workspace, &mut by_tag).await?;
        scan_citations(workspace, &mut by_tag).await?;
        scan_repositories(workspace, &mut by_tag).await?;
        scan_media(workspace, &mut by_tag).await?;
        scan_notes(workspace, &mut by_tag).await?;
        scan_dna_tests(workspace, &mut by_tag).await?;
        scan_dna_matches(workspace, &mut by_tag).await?;
        Ok(Self { by_tag })
    }

    /// The records carrying `tag`, grouped by object type (in scan order) with counts and examples.
    pub(crate) fn groups(&self, tag: TagId) -> Vec<TagUsageGroup> {
        let Some(records) = self.by_tag.get(&tag) else {
            return Vec::new();
        };
        let mut groups: Vec<TagUsageGroup> = Vec::new();
        for record in records {
            match groups.iter_mut().find(|g| g.kind == record.kind) {
                Some(group) => {
                    group.count += 1;
                    if group.examples.len() < MAX_EXAMPLES {
                        group.examples.push(record.clone());
                    }
                }
                None => groups.push(TagUsageGroup {
                    kind: record.kind,
                    count: 1,
                    examples: vec![record.clone()],
                }),
            }
        }
        groups
    }
}

/// Pushes one carrying record onto a tag's bucket.
fn push(map: &mut HashMap<TagId, Vec<UsingRecordRef>>, tag: TagId, record: UsingRecordRef) {
    map.entry(tag).or_default().push(record);
}

/// Inverts person tag applications.
async fn scan_persons(
    workspace: &Workspace,
    person_names: &HashMap<String, String>,
    map: &mut HashMap<TagId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_persons().await? {
        let (Some(id), Some(human_id)) = (view.person_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = person_names.get(&human_id).cloned();
        for tag in view.tags() {
            push(
                map,
                tag,
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

/// Inverts family tag applications.
async fn scan_families(workspace: &Workspace, map: &mut HashMap<TagId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_families().await? {
        let (Some(id), Some(human_id)) = (view.family_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        for tag in view.tags() {
            push(
                map,
                tag,
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

/// Inverts event tag applications.
async fn scan_events(workspace: &Workspace, map: &mut HashMap<TagId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_events().await? {
        let (Some(id), Some(human_id)) = (view.event_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.description().map(ToOwned::to_owned);
        for tag in view.tags() {
            push(
                map,
                tag,
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

/// Inverts place tag applications.
async fn scan_places(workspace: &Workspace, map: &mut HashMap<TagId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_places().await? {
        let (Some(id), Some(human_id)) = (view.place_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.names().first().map(|n| n.text.clone());
        for tag in view.tags() {
            push(
                map,
                tag,
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

/// Inverts source tag applications.
async fn scan_sources(workspace: &Workspace, map: &mut HashMap<TagId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_sources().await? {
        let (Some(id), Some(human_id)) = (view.source_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.title().map(ToOwned::to_owned);
        for tag in view.tags() {
            push(
                map,
                tag,
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

/// Inverts citation tag applications.
async fn scan_citations(workspace: &Workspace, map: &mut HashMap<TagId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_citations().await? {
        let (Some(id), Some(human_id)) = (view.citation_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        for tag in view.tags() {
            push(
                map,
                tag,
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

/// Inverts repository tag applications.
async fn scan_repositories(
    workspace: &Workspace,
    map: &mut HashMap<TagId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_repositories().await? {
        let (Some(id), Some(human_id)) = (view.repository_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.name().map(ToOwned::to_owned);
        for tag in view.tags() {
            push(
                map,
                tag,
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

/// Inverts media tag applications.
async fn scan_media(workspace: &Workspace, map: &mut HashMap<TagId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_media().await? {
        let (Some(id), Some(human_id)) = (view.media_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        for tag in view.tags() {
            push(
                map,
                tag,
                UsingRecordRef {
                    kind: UsingKind::Media,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: None,
                },
            );
        }
    }
    Ok(())
}

/// Inverts note tag applications.
async fn scan_notes(workspace: &Workspace, map: &mut HashMap<TagId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_notes().await? {
        let (Some(id), Some(human_id)) = (view.note_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        for tag in view.tags() {
            push(
                map,
                tag,
                UsingRecordRef {
                    kind: UsingKind::Note,
                    human_id: human_id.clone(),
                    id: id.to_string(),
                    label: None,
                },
            );
        }
    }
    Ok(())
}

/// Inverts DNA-test tag applications.
async fn scan_dna_tests(workspace: &Workspace, map: &mut HashMap<TagId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_dna_tests().await? {
        let (Some(id), Some(human_id)) = (view.dna_test_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        for tag in view.tags() {
            push(
                map,
                tag,
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

/// Inverts DNA-match tag applications.
async fn scan_dna_matches(
    workspace: &Workspace,
    map: &mut HashMap<TagId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_dna_matches().await? {
        let (Some(id), Some(human_id)) = (view.dna_match_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.predicted_relationship().map(ToOwned::to_owned);
        for tag in view.tags() {
            push(
                map,
                tag,
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
