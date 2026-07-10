//! The reverse citation index (Phase 5 PR 9): given a citation, which records use it.
//!
//! Source › Citations renders the records each citation backs ("John Smith — Birth"). Citations
//! reference a source, but no aggregate stores the inverse, so this scans every citation-bearing
//! projection once (person/event/family/place) and inverts the attachments to a
//! `CitationId -> [CitingRecordRef]` map. The join lives here in the app/db layer (the
//! cross-aggregate-joins dependency note), keeping it out of the renderer.

use std::collections::HashMap;

use genealogy_core::ids::CitationId;

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

/// Inverts person citations: row-level, names, facts (with the fact type), and associations.
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
            for citation in &name.citations {
                push(map, *citation, make(CitingContext::Name));
            }
        }
        for fact in view.facts() {
            for citation in &fact.citations {
                push(map, *citation, make(CitingContext::Fact(fact.fact.fact_type.clone())));
            }
        }
        for association in view.asserted_associations() {
            for citation in &association.citations {
                push(
                    map,
                    *citation,
                    make(CitingContext::Association(association.association.role.clone())),
                );
            }
        }
    }
    Ok(())
}

/// Inverts event citations: row-level and per-participant (with the participant role).
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
        for participant in view.asserted_participants() {
            for citation in &participant.citations {
                push(
                    map,
                    *citation,
                    make(CitingContext::Participant(participant.value.role.clone())),
                );
            }
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
            for citation in &partner.citations {
                push(map, *citation, make(CitingContext::Partner));
            }
        }
        for child in view.asserted_children() {
            for citation in &child.citations {
                push(map, *citation, make(CitingContext::Child));
            }
        }
        for event in view.asserted_linked_events() {
            for citation in &event.citations {
                push(map, *citation, make(CitingContext::FamilyEvent));
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
            for citation in &place_type.citations {
                push(map, *citation, make(CitingContext::PlaceType));
            }
        }
    }
    Ok(())
}
