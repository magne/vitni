//! The reverse DNA-inference index (PR45): given a DNA match, which Person/Family assertions cite it.
//!
//! A relationship inference is an ordinary Person/Family assertion whose provenance envelope cites a
//! `DnaMatch` as evidence (data-model §12, ADR 0023). No aggregate stores that inverse, so this scans
//! the relationship-bearing asserted values (person facts/associations/participations, family
//! partners/children/linked events), filters the `DnaMatch` evidence variant, and inverts them into a
//! `DnaMatchId -> [DnaInferenceRef]` map. Row-level citations (`view.citations()`) are direct
//! `CitationId` attachments and structurally cannot target a match, so they are not scanned. The join
//! lives here in the app/db layer, mirroring [`citation_usage`](crate::citation_usage).

use std::collections::HashMap;

use genealogy_core::ids::DnaMatchId;
use genealogy_core::provenance::{Confidence, EvidenceRef};
use genealogy_db::Store;

use crate::dna_match::DnaInferenceRef;
use crate::dto::{CitingContext, CitingKind, CitingRecordRef};
use crate::error::AppError;
use crate::person::list_persons;
use crate::workspace::Workspace;

/// A `DnaMatchId -> [DnaInferenceRef]` reverse index over the relationship-bearing projections.
pub(crate) struct DnaMatchUsage {
    by_match: HashMap<DnaMatchId, Vec<DnaInferenceRef>>,
}

impl DnaMatchUsage {
    /// Scans the relationship-bearing projections and inverts the DNA-match evidence links.
    pub(crate) async fn load(workspace: &Workspace) -> Result<Self, AppError> {
        let person_names: HashMap<String, String> = list_persons(workspace)
            .await?
            .into_iter()
            .filter_map(|p| p.display_name.map(|name| (p.human_id, name)))
            .collect();

        let mut by_match: HashMap<DnaMatchId, Vec<DnaInferenceRef>> = HashMap::new();
        scan_persons(workspace, &person_names, &mut by_match).await?;
        scan_families(workspace, &mut by_match).await?;
        Ok(Self { by_match })
    }

    /// The inferences citing `dna_match`, in scan order (empty if none).
    pub(crate) fn inferences(&self, dna_match: DnaMatchId) -> Vec<DnaInferenceRef> {
        self.by_match.get(&dna_match).cloned().unwrap_or_default()
    }
}

/// Pushes one inference per DNA match cited in `citations`, carrying the assertion's surety and its
/// documentary-citation source count (the `Citation` variants on the same assertion).
fn push_inferences(
    map: &mut HashMap<DnaMatchId, Vec<DnaInferenceRef>>,
    citations: &[EvidenceRef],
    confidence: Option<Confidence>,
    make: impl Fn() -> CitingRecordRef,
) {
    let source_count = citations.iter().filter_map(|evidence| evidence.as_citation()).count();
    for dna_match in citations.iter().filter_map(|evidence| evidence.as_dna_match()) {
        map.entry(dna_match).or_default().push(DnaInferenceRef {
            record: make(),
            confidence,
            source_count,
        });
    }
}

/// Inverts person inferences: facts (with the fact type), associations (with the role), and event
/// participations (with the participant role) — the person-owned relationship claims (data-model §12).
async fn scan_persons(
    workspace: &Workspace,
    person_names: &HashMap<String, String>,
    map: &mut HashMap<DnaMatchId, Vec<DnaInferenceRef>>,
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
        for fact in view.facts() {
            push_inferences(map, &fact.citations, fact.confidence, || {
                make(CitingContext::Fact(fact.value.fact_type.clone()))
            });
        }
        for association in view.asserted_associations() {
            push_inferences(map, &association.citations, association.confidence, || {
                make(CitingContext::Association(association.value.role.clone()))
            });
        }
        for participation in view.participations_with_assertions() {
            let asserted = &participation.value;
            push_inferences(map, &asserted.citations, asserted.confidence, || {
                make(CitingContext::Participant(asserted.value.role.clone()))
            });
        }
    }
    Ok(())
}

/// Inverts family inferences: partner and child-in-family relationships, and linked family events —
/// the family-owned relationship claims a DNA match backs (data-model §12).
async fn scan_families(
    workspace: &Workspace,
    map: &mut HashMap<DnaMatchId, Vec<DnaInferenceRef>>,
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
        for partner in view.asserted_partners() {
            push_inferences(map, &partner.citations, partner.confidence, || {
                make(CitingContext::Partner)
            });
        }
        for child in view.asserted_children() {
            push_inferences(map, &child.citations, child.confidence, || make(CitingContext::Child));
        }
        for event in view.asserted_linked_events() {
            push_inferences(map, &event.citations, event.confidence, || {
                make(CitingContext::FamilyEvent)
            });
        }
    }
    Ok(())
}

/// Resolves DNA-match `human_id`s to the [`EvidenceRef::DnaMatch`]s that back an assertion, linking
/// the provenance envelope to real `DnaMatch` aggregates (data-model §12, ADR 0023). The counterpart
/// of [`resolve_citation_refs`](crate::use_case::resolve_citation_refs) for the DNA evidence target.
///
/// # Errors
///
/// [`AppError::DnaMatchNotFound`] if a cited DNA-match `human_id` is unknown.
pub(crate) async fn resolve_dna_match_refs(store: &Store, human_ids: &[String]) -> Result<Vec<EvidenceRef>, AppError> {
    let mut refs = Vec::with_capacity(human_ids.len());
    for human_id in human_ids {
        let view = store
            .find_dna_match(human_id)
            .await?
            .ok_or_else(|| AppError::DnaMatchNotFound(human_id.clone()))?;
        let id = view
            .dna_match_id()
            .ok_or_else(|| AppError::DnaMatchNotFound(human_id.clone()))?;
        refs.push(EvidenceRef::DnaMatch(id));
    }
    Ok(refs)
}
