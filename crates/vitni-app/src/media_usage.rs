//! The reverse media index (Phase 5 PR 10): given a media object, which records reference it.
//!
//! Media › Overview renders a "Used by" card naming each record an image backs ("John Smith —
//! portrait"). Records reference media, but no aggregate stores the inverse, so this scans every
//! media-bearing projection once (person/family/event/place/source/citation) and inverts the
//! attachments to a `MediaId -> [UsingRecordRef]` map. The join lives here in the app/db layer (the
//! cross-aggregate-joins dependency note), keeping it out of the renderer.

use std::collections::HashMap;

use vitni_core::ids::MediaId;

use crate::dto::{UsingKind, UsingRecordRef};
use crate::error::AppError;
use crate::person::list_persons;
use crate::workspace::Workspace;

/// A `MediaId -> [UsingRecordRef]` reverse index over the media-bearing aggregates.
pub(crate) struct MediaUsage {
    by_media: HashMap<MediaId, Vec<UsingRecordRef>>,
}

impl MediaUsage {
    /// Scans every media-bearing projection and inverts the attachments.
    pub(crate) async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let person_names: HashMap<String, String> = list_persons(workspace)
            .await?
            .into_iter()
            .filter_map(|p| p.display_name.map(|name| (p.human_id, name)))
            .collect();

        let mut by_media: HashMap<MediaId, Vec<UsingRecordRef>> = HashMap::new();
        scan_persons(workspace, &person_names, &mut by_media).await?;
        scan_families(workspace, &mut by_media).await?;
        scan_events(workspace, &mut by_media).await?;
        scan_places(workspace, &mut by_media).await?;
        scan_sources(workspace, &mut by_media).await?;
        scan_citations(workspace, &mut by_media).await?;
        Ok(Self { by_media })
    }

    /// The records that reference `media`, in scan order (empty if none).
    pub(crate) fn used_by(&self, media: MediaId) -> Vec<UsingRecordRef> {
        self.by_media.get(&media).cloned().unwrap_or_default()
    }
}

/// Pushes one referencing record onto a media object's bucket.
fn push(map: &mut HashMap<MediaId, Vec<UsingRecordRef>>, media: MediaId, record: UsingRecordRef) {
    map.entry(media).or_default().push(record);
}

/// Inverts person media attachments.
async fn scan_persons(
    workspace: &Workspace,
    person_names: &HashMap<String, String>,
    map: &mut HashMap<MediaId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_persons().await? {
        let (Some(id), Some(human_id)) = (view.person_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = person_names.get(&human_id).cloned();
        for media in view.media() {
            push(
                map,
                media.media_id,
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

/// Inverts family media attachments.
async fn scan_families(workspace: &Workspace, map: &mut HashMap<MediaId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_families().await? {
        let (Some(id), Some(human_id)) = (view.family_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        for media in view.media() {
            push(
                map,
                media.media_id,
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

/// Inverts event media attachments.
async fn scan_events(workspace: &Workspace, map: &mut HashMap<MediaId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_events().await? {
        let (Some(id), Some(human_id)) = (view.event_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.description().map(ToOwned::to_owned);
        for media in view.media() {
            push(
                map,
                media.media_id,
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

/// Inverts place media attachments.
async fn scan_places(workspace: &Workspace, map: &mut HashMap<MediaId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_places().await? {
        let (Some(id), Some(human_id)) = (view.place_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.names().first().map(|n| n.text.clone());
        for media in view.media() {
            push(
                map,
                media.media_id,
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

/// Inverts source media attachments.
async fn scan_sources(workspace: &Workspace, map: &mut HashMap<MediaId, Vec<UsingRecordRef>>) -> Result<(), AppError> {
    for view in workspace.store().list_sources().await? {
        let (Some(id), Some(human_id)) = (view.source_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.title().map(ToOwned::to_owned);
        for media in view.media() {
            push(
                map,
                media.media_id,
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

/// Inverts citation media attachments.
async fn scan_citations(
    workspace: &Workspace,
    map: &mut HashMap<MediaId, Vec<UsingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_citations().await? {
        let (Some(id), Some(human_id)) = (view.citation_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        for media in view.media() {
            push(
                map,
                media.media_id,
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
