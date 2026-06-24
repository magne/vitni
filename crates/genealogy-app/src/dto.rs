//! Shared cross-aggregate reference DTOs and their joins (ADR 0006).
//!
//! `*Summary` DTOs reference related aggregates by their stable `id` **and** their user-facing
//! `human_id`, so a frontend can navigate by the stable id rather than the mutable `human_id` (the
//! cross-aggregate-joins dependency note; ADR 0004 §2). These types are shared by more than one
//! aggregate's summary (family/event/place), so they live here rather than in any one module.

use std::collections::HashMap;

use genealogy_core::ids::{CitationId, MediaId, TagId};
use genealogy_core::provenance::{Confidence, EvidenceAnalysis};
use genealogy_db::Store;

use crate::citation::TagRef;
use crate::error::AppError;

/// A reference to a related aggregate, carrying both its user-facing `human_id` (the display label)
/// and its stable aggregate `id` (a UUID string) so a frontend can join/navigate by the stable id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AggRef {
    /// The referenced aggregate's user-facing identifier (e.g. `C0001`).
    pub human_id: String,
    /// The referenced aggregate's stable id (a UUID string) — the join/navigation key.
    pub id: String,
}

/// A media object attached to an aggregate, with its per-use caption for the gallery.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MediaRefSummary {
    /// The media object's user-facing identifier (e.g. `O0001`).
    pub human_id: String,
    /// The media object's stable `MediaId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The per-use caption, if set.
    pub caption: Option<String>,
}

/// A citation attached to an aggregate, joined to the Citation (and its Source) projection so the
/// detail tabs can render source · page · surety · the Evidence Explained axes, while navigating by
/// the stable citation/source ids.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CitationRef {
    /// The citation's user-facing identifier (e.g. `C0001`).
    pub human_id: String,
    /// The citation's stable `CitationId` (a UUID string) — the join/navigation key.
    pub id: String,
    /// The cited source (its `human_id` + stable id), for display and navigation.
    pub source: Option<AggRef>,
    /// The cited source's title, for display.
    pub source_title: Option<String>,
    /// The page / locator within the source, if set.
    pub page: Option<String>,
    /// The operator's confidence in this citation. Structured so the frontend localizes it.
    pub confidence: Option<Confidence>,
    /// The citation's Evidence Explained analysis (the three axes), if set.
    pub analysis: Option<EvidenceAnalysis>,
}

/// Builds a `CitationId -> CitationRef` lookup from the Citation projection, joined to the Source
/// projection for the source label, so an owner's attached citations resolve to a full row without a
/// per-citation query (the cross-aggregate join lives here — the app/db layer).
pub(crate) async fn citation_refs(store: &Store) -> Result<HashMap<CitationId, CitationRef>, AppError> {
    let sources: HashMap<_, (String, Option<String>)> = store
        .list_sources()
        .await?
        .iter()
        .filter_map(|s| {
            Some((
                s.source_id()?,
                (s.human_id()?.as_str().to_owned(), s.title().map(ToOwned::to_owned)),
            ))
        })
        .collect();

    let mut map = HashMap::new();
    for view in store.list_citations().await? {
        let (Some(id), Some(human_id)) = (view.citation_id(), view.human_id()) else {
            continue;
        };
        let source = view.source_id().and_then(|sid| {
            sources.get(&sid).map(|(human, _)| AggRef {
                human_id: human.clone(),
                id: sid.to_string(),
            })
        });
        let source_title = view
            .source_id()
            .and_then(|sid| sources.get(&sid))
            .and_then(|(_, title)| title.clone());
        map.insert(
            id,
            CitationRef {
                human_id: human_id.as_str().to_owned(),
                id: id.to_string(),
                source,
                source_title,
                page: view.page().map(ToOwned::to_owned),
                confidence: view.confidence().copied(),
                analysis: view.evidence_analysis().copied(),
            },
        );
    }
    Ok(map)
}

/// Loads a `MediaId -> (human_id, id)` lookup from the Media projection; the per-use caption is
/// supplied by the owning aggregate's `MediaRef`, not the media object itself.
pub(crate) async fn media_refs(store: &Store) -> Result<HashMap<MediaId, (String, String)>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_media().await? {
        if let (Some(id), Some(human_id)) = (view.media_id(), view.human_id()) {
            map.insert(id, (human_id.as_str().to_owned(), id.to_string()));
        }
    }
    Ok(map)
}

/// Builds a `TagId -> TagRef` lookup from the Tag projection, to render applied tags by name/colour/
/// priority (never by id — data-model §9).
pub(crate) async fn tag_refs(store: &Store) -> Result<HashMap<TagId, TagRef>, AppError> {
    let mut map = HashMap::new();
    for view in store.list_tags().await? {
        if let (Some(id), Some(name)) = (view.tag_id(), view.name()) {
            map.insert(
                id,
                TagRef {
                    id: id.to_string(),
                    name: name.to_owned(),
                    color: view.color().map(ToOwned::to_owned),
                    priority: view.priority(),
                },
            );
        }
    }
    Ok(map)
}
