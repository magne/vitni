//! The reverse citation index (Phase 5 PR 9): given a citation, which records use it.
//!
//! Source › Citations renders the records each citation backs ("John Smith — Birth"). Citations
//! reference a source, but no aggregate stores the inverse, so this scans every citation-bearing
//! projection once (person/event/family/place) and inverts the attachments to a
//! `CitationId -> [CitingRecordRef]` map. The join lives here in the app/db layer (the
//! cross-aggregate-joins dependency note), keeping it out of the renderer.

use std::collections::HashMap;

use genealogy_core::ids::CitationId;
use genealogy_db::Store;

use crate::dto::{CitingContext, CitingKind, CitingRecordRef};
use crate::error::AppError;
use crate::person::list_persons;
use crate::workspace::Workspace;

/// A `CitationId -> [CitingRecordRef]` reverse index over the citation-bearing aggregates.
pub(crate) struct CitationUsage {
    by_citation: HashMap<CitationId, Vec<CitingRecordRef>>,
}

impl CitationUsage {
    /// Scans every citation-bearing projection and inverts the attachments.
    pub(crate) async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let person_names: HashMap<String, String> = list_persons(workspace)
            .await?
            .into_iter()
            .filter_map(|p| p.display_name.map(|name| (p.human_id, name)))
            .collect();

        let mut by_citation: HashMap<CitationId, Vec<CitingRecordRef>> = HashMap::new();
        scan_persons(workspace, &person_names, &mut by_citation).await?;
        scan_events(workspace, &mut by_citation).await?;
        scan_families(workspace, &mut by_citation).await?;
        scan_places(workspace, &mut by_citation).await?;
        Ok(Self { by_citation })
    }

    /// The records that use `citation`, in scan order (empty if none).
    pub(crate) fn backers(&self, citation: CitationId) -> Vec<CitingRecordRef> {
        self.by_citation.get(&citation).cloned().unwrap_or_default()
    }
}

/// Pushes one backer onto a citation's bucket.
fn push(map: &mut HashMap<CitationId, Vec<CitingRecordRef>>, citation: CitationId, record: CitingRecordRef) {
    map.entry(citation).or_default().push(record);
}

/// A `CitationId -> backers count` map — the Citations tab's "Backs" column, matching the count
/// [`CitationUsage::count`] would return but without resolving person-name labels (so it reads only
/// the `store`, never [`list_persons`], keeping it off the person-summary recursion path).
///
/// Every occurrence the [`CitationUsage`] scanners push must be counted here too, so the two stay in
/// sync (Backs == `SourceCitationRef.backers.len()`).
pub(crate) async fn citation_backs_counts(store: &Store) -> Result<HashMap<CitationId, usize>, AppError> {
    let mut counts: HashMap<CitationId, usize> = HashMap::new();
    let mut bump = |citation: CitationId| *counts.entry(citation).or_default() += 1;
    for view in store.list_persons().await? {
        for citation in view.citations() {
            bump(citation);
        }
        for name in view.asserted_names() {
            for citation in name.citations.iter().filter_map(|e| e.as_citation()) {
                bump(citation);
            }
        }
        for fact in view.facts() {
            for citation in fact.citations.iter().filter_map(|e| e.as_citation()) {
                bump(citation);
            }
        }
        for association in view.asserted_associations() {
            for citation in association.citations.iter().filter_map(|e| e.as_citation()) {
                bump(citation);
            }
        }
        for participation in view.participations_with_assertions() {
            for citation in participation.value.citations.iter().filter_map(|e| e.as_citation()) {
                bump(citation);
            }
        }
    }
    for view in store.list_events().await? {
        for citation in view.citations() {
            bump(citation);
        }
    }
    for view in store.list_families().await? {
        for citation in view.citations() {
            bump(citation);
        }
        for partner in view.asserted_partners() {
            for citation in partner.citations.iter().filter_map(|e| e.as_citation()) {
                bump(citation);
            }
        }
        for child in view.asserted_children() {
            for citation in child.citations.iter().filter_map(|e| e.as_citation()) {
                bump(citation);
            }
        }
        for event in view.asserted_linked_events() {
            for citation in event.citations.iter().filter_map(|e| e.as_citation()) {
                bump(citation);
            }
        }
    }
    for view in store.list_places().await? {
        for citation in view.citations() {
            bump(citation);
        }
        if let Some(place_type) = view.asserted_place_type() {
            for citation in place_type.citations.iter().filter_map(|e| e.as_citation()) {
                bump(citation);
            }
        }
    }
    Ok(counts)
}

/// Inverts person citations: row-level, names, facts (with the fact type), associations, and
/// participations (with the participant role — the canonical, person-owned side; data-model §6, §10).
async fn scan_persons(
    workspace: &Workspace,
    person_names: &HashMap<String, String>,
    map: &mut HashMap<CitationId, Vec<CitingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_persons().await? {
        let (Some(id), Some(human_id)) = (view.person_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = person_names.get(&human_id).cloned();
        let make = |context| CitingRecordRef {
            kind: CitingKind::Person,
            human_id: human_id.clone(),
            id: id.to_string(),
            label: label.clone(),
            context,
        };
        for citation in view.citations() {
            push(map, citation, make(CitingContext::Record));
        }
        for name in view.asserted_names() {
            for citation in name.citations.iter().filter_map(|e| e.as_citation()) {
                push(map, citation, make(CitingContext::Name));
            }
        }
        for fact in view.facts() {
            for citation in fact.citations.iter().filter_map(|e| e.as_citation()) {
                push(map, citation, make(CitingContext::Fact(fact.value.fact_type.clone())));
            }
        }
        for association in view.asserted_associations() {
            for citation in association.citations.iter().filter_map(|e| e.as_citation()) {
                push(
                    map,
                    citation,
                    make(CitingContext::Association(association.value.role.clone())),
                );
            }
        }
        for participation in view.participations_with_assertions() {
            let asserted = &participation.value;
            for citation in asserted.citations.iter().filter_map(|e| e.as_citation()) {
                push(
                    map,
                    citation,
                    make(CitingContext::Participant(asserted.value.role.clone())),
                );
            }
        }
    }
    Ok(())
}

/// Inverts event citations: row-level.
async fn scan_events(
    workspace: &Workspace,
    map: &mut HashMap<CitationId, Vec<CitingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_events().await? {
        let (Some(id), Some(human_id)) = (view.event_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.description().map(ToOwned::to_owned);
        let make = |context| CitingRecordRef {
            kind: CitingKind::Event,
            human_id: human_id.clone(),
            id: id.to_string(),
            label: label.clone(),
            context,
        };
        for citation in view.citations() {
            push(map, citation, make(CitingContext::Record));
        }
    }
    Ok(())
}

/// Inverts family citations: row-level, partners, children, and linked family events.
async fn scan_families(
    workspace: &Workspace,
    map: &mut HashMap<CitationId, Vec<CitingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_families().await? {
        let (Some(id), Some(human_id)) = (view.family_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let make = |context| CitingRecordRef {
            kind: CitingKind::Family,
            human_id: human_id.clone(),
            id: id.to_string(),
            label: None,
            context,
        };
        for citation in view.citations() {
            push(map, citation, make(CitingContext::Record));
        }
        for partner in view.asserted_partners() {
            for citation in partner.citations.iter().filter_map(|e| e.as_citation()) {
                push(map, citation, make(CitingContext::Partner));
            }
        }
        for child in view.asserted_children() {
            for citation in child.citations.iter().filter_map(|e| e.as_citation()) {
                push(map, citation, make(CitingContext::Child));
            }
        }
        for event in view.asserted_linked_events() {
            for citation in event.citations.iter().filter_map(|e| e.as_citation()) {
                push(map, citation, make(CitingContext::FamilyEvent));
            }
        }
    }
    Ok(())
}

/// Inverts place citations: row-level and the place-type assertion.
async fn scan_places(
    workspace: &Workspace,
    map: &mut HashMap<CitationId, Vec<CitingRecordRef>>,
) -> Result<(), AppError> {
    for view in workspace.store().list_places().await? {
        let (Some(id), Some(human_id)) = (view.place_id(), view.human_id()) else {
            continue;
        };
        let human_id = human_id.as_str().to_owned();
        let label = view.names().first().map(|n| n.text.clone());
        let make = |context| CitingRecordRef {
            kind: CitingKind::Place,
            human_id: human_id.clone(),
            id: id.to_string(),
            label: label.clone(),
            context,
        };
        for citation in view.citations() {
            push(map, citation, make(CitingContext::Record));
        }
        if let Some(place_type) = view.asserted_place_type() {
            for citation in place_type.citations.iter().filter_map(|e| e.as_citation()) {
                push(map, citation, make(CitingContext::PlaceType));
            }
        }
    }
    Ok(())
}
