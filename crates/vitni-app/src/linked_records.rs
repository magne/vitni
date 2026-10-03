//! A person's *Linked records* (ADR 0039 §5): every record of its cluster with the import record it
//! came from (ADR 0037), the origin of each of the cluster's claims for *Why we believe*, and
//! *Unlink*, which retracts the merge that put a record in the cluster.

use std::collections::{BTreeMap, HashMap};

use vitni_core::enums::EvidenceLevel;
use vitni_core::ids::{ImportRunId, PersonId};
use vitni_core::origin::RecordOrigin;
use vitni_core::person::{PersonError, PersonView};

use crate::dto::AggRef;
use crate::error::AppError;
use crate::identity::{ClusterView, PersonClusters, views};
use crate::import_run::{ImportRunSummary, find_import_run};
use crate::person::render_name;
use crate::session::Session;
use crate::use_case::Provenance;
use crate::workspace::Workspace;

/// The dataset scheme whose record ids are also paths on its website.
const DIGITALARKIVET: &str = "digitalarkivet";

/// The import record a record or claim came from, with the labels its run recorded.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OriginRef {
    /// The dataset record (ADR 0037 §1).
    pub origin: RecordOrigin,
    /// The dataset's label as the importing run recorded it; `None` if the run is not in the log.
    pub dataset_label: Option<String>,
    /// What the importing run imported (a file name, an assisted session's request).
    pub source_label: Option<String>,
    /// The record's page on the dataset's website, when the dataset has one ([`record_url`]).
    pub url: Option<String>,
}

/// One record of a person's cluster.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedRecord {
    /// The record.
    pub record: AggRef,
    /// Its own primary name, if it has one.
    pub display_name: Option<String>,
    /// Whether it is an imported persona or a conclusion.
    pub evidence_level: EvidenceLevel,
    /// The import record that created it; `None` for a record entered by hand.
    pub origin: Option<OriginRef>,
    /// The member whose merge linked it, when that is not the root — unlinking that member takes this
    /// record along.
    pub via: Option<String>,
    /// Whether this is the cluster's root, the record the others are linked to.
    pub root: bool,
}

/// A person's cluster as its *Linked records* view and *Why we believe* read it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkedRecords {
    /// The root first, then every member in id order.
    pub records: Vec<LinkedRecord>,
    /// The origin of every imported live claim of the cluster, by the claim's `AssertionId`.
    pub claim_origins: BTreeMap<String, OriginRef>,
}

/// The record's page on its dataset's website, or `None` for a dataset with no URL form (ADR 0037 §2).
/// A file's lineage has none. Digitalarkivet redirects a record id at the site's root to the record's
/// page, census (`pf…`) and church book (`pd…`) alike.
#[must_use]
pub fn record_url(origin: &RecordOrigin) -> Option<String> {
    (origin.dataset.as_str() == DIGITALARKIVET && !origin.record.is_empty())
        .then(|| format!("https://www.digitalarkivet.no/{}", origin.record))
}

/// Every record of `human_id`'s cluster, its root first, with the origin of each record and claim.
/// A member resolves to its root, so any record of the cluster reads the same view.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if no person has `human_id`, or a store error.
pub async fn linked_records(workspace: &Workspace, human_id: &str) -> Result<LinkedRecords, AppError> {
    let store = workspace.store();
    let id = person_id(workspace, human_id).await?;
    let clusters = PersonClusters::load(store).await?;
    let root = clusters.root(id);
    let records = views(store, &clusters.cluster(root)).await?;
    let holders = edge_holders(&records);
    let mut runs = Runs::default();
    let mut linked = LinkedRecords {
        records: Vec::with_capacity(records.len()),
        claim_origins: BTreeMap::new(),
    };
    for view in &records {
        let Some(record_id) = view.person_id() else { continue };
        let aggregate_id = record_id.to_string();
        for (assertion, claim) in store.assertion_origins("person", &aggregate_id).await? {
            let claim = runs.origin_ref(workspace, claim).await?;
            linked.claim_origins.insert(assertion.to_string(), claim);
        }
        let origin = match store.created_origin("person", &aggregate_id).await? {
            Some(created) => Some(runs.origin_ref(workspace, created).await?),
            None => None,
        };
        let via = holders
            .get(&record_id)
            .filter(|holder| **holder != root)
            .and_then(|holder| human_id_of(&records, *holder));
        linked.records.push(LinkedRecord {
            record: AggRef {
                human_id: view.human_id().map(|id| id.as_str().to_owned()).unwrap_or_default(),
                id: aggregate_id,
            },
            display_name: view.names().first().map(|name| render_name(name)),
            evidence_level: view.evidence_level().unwrap_or(EvidenceLevel::Conclusion),
            origin,
            via,
            root: record_id == root,
        });
    }
    Ok(linked)
}

/// Unlinks `member_human_id` from `human_id`'s cluster by retracting the merge that linked it, on the
/// stream of whichever record of the cluster holds it (ADR 0039 §4). The member becomes its own
/// person again, together with every record linked through it.
///
/// # Errors
///
/// [`AppError::PersonNotFound`] if either `human_id` is unknown, [`AppError::Domain`] with
/// [`PersonError::NotLinked`] if the member is not merged into the cluster (the root itself included),
/// or a store error.
pub async fn unlink_person(
    workspace: &Workspace,
    session: &Session,
    human_id: &str,
    member_human_id: &str,
    rationale: Option<String>,
) -> Result<(), AppError> {
    let store = workspace.store();
    let id = person_id(workspace, human_id).await?;
    let member = person_id(workspace, member_human_id).await?;
    let clusters = PersonClusters::load(store).await?;
    let root = clusters.root(id);
    let not_linked = || {
        AppError::Domain(PersonError::NotLinked {
            person: root,
            other: member,
        })
    };
    if !clusters.members(root).contains(&member) {
        return Err(not_linked());
    }
    let records = views(store, &clusters.cluster(root)).await?;
    let (holder, target) = records
        .iter()
        .find_map(|view| {
            let edge = view.merged_with_assertions().iter().find(|edge| edge.value == member)?;
            Some((view.person_id()?, edge.assertion_id))
        })
        .ok_or_else(not_linked)?;
    let provenance = Provenance {
        rationale,
        ..Provenance::default()
    };
    PersonView::retract(store, session, holder, target, provenance).await
}

/// The person id of `human_id`.
async fn person_id(workspace: &Workspace, human_id: &str) -> Result<PersonId, AppError> {
    workspace
        .store()
        .find_person(human_id)
        .await?
        .and_then(|view| view.person_id())
        .ok_or_else(|| AppError::PersonNotFound(human_id.to_owned()))
}

/// Which record's stream holds the merge of each member.
fn edge_holders(records: &[PersonView]) -> HashMap<PersonId, PersonId> {
    let mut holders = HashMap::new();
    for view in records {
        let Some(holder) = view.person_id() else { continue };
        for edge in view.merged_with_assertions() {
            holders.insert(edge.value, holder);
        }
    }
    holders
}

/// The `human_id` of `id` among `records`.
fn human_id_of(records: &[PersonView], id: PersonId) -> Option<String> {
    records
        .iter()
        .find(|view| view.person_id() == Some(id))
        .and_then(|view| view.human_id().map(|id| id.as_str().to_owned()))
}

/// The import runs read so far, each once.
#[derive(Default)]
struct Runs(HashMap<ImportRunId, Option<ImportRunSummary>>);

impl Runs {
    /// `origin` with its run's labels and its record's URL.
    async fn origin_ref(&mut self, workspace: &Workspace, origin: RecordOrigin) -> Result<OriginRef, AppError> {
        let run = if let Some(run) = self.0.get(&origin.run) {
            run.clone()
        } else {
            let run = find_import_run(workspace, origin.run).await?;
            self.0.insert(origin.run, run.clone());
            run
        };
        Ok(OriginRef {
            url: record_url(&origin),
            dataset_label: run.as_ref().map(|run| run.dataset_label.clone()),
            source_label: run.map(|run| run.source_label),
            origin,
        })
    }
}
