//! Planning an import (ADR 0040 §2): every staged entity's disposition, decided before anything is
//! written.
//!
//! Resolution runs in four passes over the whole plan:
//!
//! 1. **Deterministic.** An entity resolves by its origin (ADR 0037 §4) — onto the record an earlier run
//!    of this dataset created from it, or onto the record a run recorded resolving it to — then a
//!    person or family by an external id, and a tag by its case-folded name (ADR 0038 §6).
//! 2. **Scope.** A record whose own person or family resolved onto a record another dataset made keeps
//!    that dataset's contents: only its identity is written — the name and sex, the family's members —
//!    and the rest of its graph is withheld, as is any entity only withheld links reach.
//! 3. **Dry run.** A record this dataset made is unchanged when its writes, run through the origin
//!    gate's dry run, write nothing; otherwise it is an update naming the fields they assert.
//! 4. **Candidates.** A person, place, source or repository nothing resolved is matched against the
//!    workspace; its relatives already resolved stand in as the records they resolved onto, so a
//!    confirmed father raises his child's match. Candidates are only ever stored records, so two items
//!    of one graph are never matched to each other, and a record a sibling resolved onto is never one.

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use uuid::Uuid;
use vitni_core::import_run::ResolvedItem;
use vitni_core::matching::{MatchBand, MatchableKind};
use vitni_core::provenance::Timestamp;

use crate::ResolutionDecision;
use crate::dto::AggRef;
use crate::error::AppError;
use crate::origin_gate::DryRun;
use crate::session::Session;
use crate::similar::SimilarRecord;
use crate::staging::candidates;
use crate::staging::graph::{EntityFields, EntityRef, GraphError, LinkKind, LocalId, RecordGraph};
use crate::staging::write::{LinkError, Resolve, Writer};
use crate::use_case::Provenance;
use crate::workspace::Workspace;

/// What the commit does with one staged entity.
#[derive(Debug, Clone, PartialEq)]
pub enum Disposition {
    /// This dataset's record from the same origin, already holding everything the entity says.
    Unchanged {
        /// The record.
        target: AggRef,
    },
    /// This dataset's record from the same origin, which the entity adds to or changes.
    Update {
        /// The record.
        target: AggRef,
        /// The fields the entity's writes assert (`person.FactAsserted.Occupation`).
        fields: Vec<String>,
    },
    /// A record identity was established for deterministically.
    Link {
        /// The record.
        target: AggRef,
        /// How it was established.
        basis: LinkBasis,
    },
    /// The record an earlier entity of the plan is — one with the same external id, or a tag of the
    /// same case-folded name — which the commit writes first.
    Duplicate {
        /// The earlier entity's index in [`ImportPlan::entities`].
        of: usize,
    },
    /// Records the engine judged possibly the same; committed as new, with the pairs left for review.
    Candidates(Vec<SimilarRecord>),
    /// Nothing resolved: a new record.
    New,
}

impl Disposition {
    /// The existing record the entity resolved onto, if any.
    #[must_use]
    pub fn target(&self) -> Option<&AggRef> {
        match self {
            Self::Unchanged { target } | Self::Update { target, .. } | Self::Link { target, .. } => Some(target),
            Self::Duplicate { .. } | Self::Candidates(_) | Self::New => None,
        }
    }
}

/// How a [`Disposition::Link`] was established.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LinkBasis {
    /// An earlier run recorded resolving this origin onto the record.
    Recorded,
    /// The entity's external id names the record.
    ExternalId,
    /// A tag of the same case-folded name.
    TagName,
}

/// Which of an entity's or link's writes the commit makes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WriteScope {
    /// Every write.
    Full,
    /// A record resolved onto another dataset's: only its identity (a person's name and sex).
    Identity,
    /// Nothing: the record's contents belong to the dataset that made the record it resolved onto.
    Withheld,
}

/// One staged entity's plan.
#[derive(Debug, Clone, PartialEq)]
pub struct PlannedEntity {
    /// The index of its graph in [`ImportPlan::graphs`].
    pub graph: usize,
    /// Its local id in that graph.
    pub local_id: LocalId,
    /// Its kind.
    pub kind: MatchableKind,
    /// What the commit does with it.
    pub disposition: Disposition,
    /// Which of its writes the commit makes.
    pub scope: WriteScope,
}

/// One staged link's plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PlannedLink {
    /// The index of its graph in [`ImportPlan::graphs`].
    pub graph: usize,
    /// Its index in that graph's links.
    pub index: usize,
    /// Whether the commit writes it.
    pub scope: WriteScope,
    /// Whether it writes anything: `false` when both ends exist and it is already on record.
    pub writes: bool,
    /// Whether an end names nothing the plan or the workspace holds; such a link is never written.
    pub dangling: bool,
}

/// Where a reference of the plan points.
#[derive(Debug, Clone, PartialEq, Eq, Hash)]
pub(crate) enum Endpoint {
    /// An entity of the plan, by its index in [`ImportPlan::entities`].
    Planned(usize),
    /// A record of the workspace.
    Stored {
        kind: MatchableKind,
        human_id: String,
        aggregate_id: String,
    },
    /// Nothing.
    Dangling,
}

/// The plan of an import: every graph submitted, and what the commit does with each entity and link.
#[derive(Debug, Clone)]
pub struct ImportPlan {
    /// The graphs, in submission order.
    pub graphs: Vec<RecordGraph>,
    /// Every entity, graph by graph in entity order.
    pub entities: Vec<PlannedEntity>,
    /// Every link, graph by graph in link order.
    pub links: Vec<PlannedLink>,
    /// Where each reference of each graph points.
    pub(crate) endpoints: HashMap<(usize, EntityRef), Endpoint>,
    /// The items resolved onto existing records this plan establishes, recorded on the run.
    pub(crate) resolved: Vec<ResolvedItem>,
    /// The document's own export date, which single-valued fields are reconciled by (ADR 0029 §2).
    pub(crate) file_asserted_at: Option<Timestamp>,
}

/// Why an import could not be planned.
#[derive(Debug, thiserror::Error)]
pub enum PlanError {
    /// A graph is malformed: the importer's fault.
    #[error(transparent)]
    Graph(#[from] GraphError),
    /// The workspace could not be read.
    #[error(transparent)]
    App(#[from] AppError),
}

impl ImportPlan {
    /// The plan of the entity `local_id` of the graph `graph`.
    #[must_use]
    pub fn entity(&self, graph: usize, local_id: LocalId) -> Option<&PlannedEntity> {
        self.entities
            .iter()
            .find(|entity| entity.graph == graph && entity.local_id == local_id)
    }

    /// The number of entities of each disposition: unchanged, updated, linked, with candidates, new —
    /// counting only the entities the commit writes.
    #[must_use]
    pub fn counts(&self) -> PlanCounts {
        let mut counts = PlanCounts::default();
        for entity in &self.entities {
            if entity.scope == WriteScope::Withheld {
                counts.withheld += 1;
                continue;
            }
            match entity.disposition {
                Disposition::Unchanged { .. } => counts.unchanged += 1,
                Disposition::Update { .. } => counts.updated += 1,
                Disposition::Link { .. } | Disposition::Duplicate { .. } => counts.linked += 1,
                Disposition::Candidates(_) => counts.candidates += 1,
                Disposition::New => counts.new += 1,
            }
        }
        counts
    }

    /// The staged entity a planned one stands for.
    pub(crate) fn staged(&self, index: usize) -> Option<(&RecordGraph, &crate::staging::graph::StagedEntity)> {
        let planned = self.entities.get(index)?;
        let graph = self.graphs.get(planned.graph)?;
        let entity = graph
            .entities
            .iter()
            .find(|entity| entity.local_id == planned.local_id)?;
        Some((graph, entity))
    }

    /// Where `reference`, made in graph `graph`, points.
    pub(crate) fn endpoint(&self, graph: usize, reference: &EntityRef) -> &Endpoint {
        self.endpoints
            .get(&(graph, reference.clone()))
            .unwrap_or(&Endpoint::Dangling)
    }
}

/// How many entities a plan gives each disposition.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct PlanCounts {
    /// Already on record.
    pub unchanged: u32,
    /// Added to or changed.
    pub updated: u32,
    /// Resolved onto a record deterministically.
    pub linked: u32,
    /// New, with possible matches to review.
    pub candidates: u32,
    /// New.
    pub new: u32,
    /// Not written: another dataset's record.
    pub withheld: u32,
}

/// The human ids a plan's references resolve to: those of existing records, and those `ids` holds for
/// planned entities (`ids[i]` for entity `i`).
pub(crate) struct Resolver<'a> {
    pub plan: &'a ImportPlan,
    pub graph: usize,
    pub ids: &'a [Option<String>],
}

impl Resolve for Resolver<'_> {
    fn human_id(&self, reference: &EntityRef) -> Option<String> {
        match self.plan.endpoint(self.graph, reference) {
            Endpoint::Planned(index) => self.ids.get(*index).cloned().flatten(),
            Endpoint::Stored { human_id, .. } => Some(human_id.clone()),
            Endpoint::Dangling => None,
        }
    }

    fn kind(&self, reference: &EntityRef) -> Option<MatchableKind> {
        match self.plan.endpoint(self.graph, reference) {
            Endpoint::Planned(index) => self.plan.entities.get(*index).map(|entity| entity.kind),
            Endpoint::Stored { kind, .. } => Some(*kind),
            Endpoint::Dangling => None,
        }
    }
}

/// Plans importing `graphs` into `workspace`, as `session` — whose import run, if any, names the
/// dataset origins resolve in (ADR 0037 §4). Outside a run nothing resolves by origin.
/// `file_asserted_at` is the document's own export date (ADR 0029 §2), if it has one.
///
/// # Errors
///
/// [`PlanError::Graph`] if a graph is malformed or two graphs stage one origin, or
/// [`PlanError::App`] if the workspace or its matching data cannot be read.
pub async fn plan_import(
    workspace: &Workspace,
    session: &Session,
    graphs: Vec<RecordGraph>,
    file_asserted_at: Option<Timestamp>,
) -> Result<ImportPlan, PlanError> {
    let mut planner = Planner::new(workspace, session, graphs)?;
    planner.plan.file_asserted_at = file_asserted_at;
    planner.resolve().await?;
    planner.resolve_endpoints().await?;
    planner.scope();
    planner.dry_run().await?;
    planner.match_candidates().await?;
    Ok(planner.plan)
}

/// A plan under construction.
struct Planner<'a> {
    workspace: &'a Workspace,
    session: &'a Session,
    plan: ImportPlan,
    /// Each entity's index by its graph and local id.
    by_local: HashMap<(usize, LocalId), usize>,
    /// Each entity's index by its origin.
    by_origin: HashMap<(MatchableKind, String, Option<String>), usize>,
}

impl<'a> Planner<'a> {
    fn new(workspace: &'a Workspace, session: &'a Session, graphs: Vec<RecordGraph>) -> Result<Self, GraphError> {
        let mut entities = Vec::new();
        let mut by_local = HashMap::new();
        let mut by_origin = HashMap::new();
        let mut links = Vec::new();
        for (graph_index, graph) in graphs.iter().enumerate() {
            graph.validate()?;
            for entity in &graph.entities {
                let kind = entity.fields.kind();
                let index = entities.len();
                let origin = (kind, graph.record.clone(), entity.item.clone());
                if by_origin.insert(origin, index).is_some() {
                    return Err(GraphError::DuplicateItem {
                        record: graph.record.clone(),
                        kind,
                        item: entity.item.clone(),
                    });
                }
                by_local.insert((graph_index, entity.local_id), index);
                entities.push(PlannedEntity {
                    graph: graph_index,
                    local_id: entity.local_id,
                    kind,
                    disposition: Disposition::New,
                    scope: WriteScope::Full,
                });
            }
            for index in 0..graph.links.len() {
                links.push(PlannedLink {
                    graph: graph_index,
                    index,
                    scope: WriteScope::Full,
                    writes: true,
                    dangling: false,
                });
            }
        }
        let plan = ImportPlan {
            graphs,
            entities,
            links,
            endpoints: HashMap::new(),
            resolved: Vec::new(),
            file_asserted_at: None,
        };
        Ok(Self {
            workspace,
            session,
            plan,
            by_local,
            by_origin,
        })
    }

    /// The dataset origins resolve in, when the session writes an import run.
    fn dataset(&self) -> Option<&vitni_core::origin::DatasetId> {
        self.session.import_run().map(|run| run.dataset())
    }

    /// The deterministic pass: by origin, then by external id, then a tag by its name.
    async fn resolve(&mut self) -> Result<(), AppError> {
        let tags = tag_names(self.workspace).await?;
        // The identities of the plan's new entities so far, so a later one with the same is the same.
        let mut identities: HashMap<String, usize> = HashMap::new();
        for index in 0..self.plan.entities.len() {
            let Some((graph, entity)) = self.plan.staged(index) else {
                continue;
            };
            let (record, item, kind) = (graph.record.clone(), entity.item.clone(), entity.fields.kind());
            if let Some((target, created)) = self.by_origin(kind, &record, item.as_deref()).await? {
                self.plan.entities[index].disposition = if created {
                    Disposition::Unchanged { target }
                } else {
                    Disposition::Link {
                        target,
                        basis: LinkBasis::Recorded,
                    }
                };
                continue;
            }
            let found = match &entity.fields {
                EntityFields::Person(person) => {
                    let found = self.by_external_id(kind, &person.external_ids).await?;
                    found.map(|target| (target, LinkBasis::ExternalId))
                }
                EntityFields::Family(family) => {
                    let found = self.by_external_id(kind, &family.external_ids).await?;
                    found.map(|target| (target, LinkBasis::ExternalId))
                }
                EntityFields::Tag(tag) => tags.get(&fold(&tag.name)).map(|id| {
                    (
                        AggRef {
                            human_id: id.clone(),
                            id: id.clone(),
                        },
                        LinkBasis::TagName,
                    )
                }),
                EntityFields::Event(_)
                | EntityFields::Place(_)
                | EntityFields::Source(_)
                | EntityFields::Citation(_)
                | EntityFields::Media(_)
                | EntityFields::Note(_)
                | EntityFields::Repository(_) => None,
            };
            let Some((target, basis)) = found else {
                let keys = identity_keys(&entity.fields);
                if let Some(of) = keys.iter().find_map(|key| identities.get(key)) {
                    self.plan.entities[index].disposition = Disposition::Duplicate { of: *of };
                } else {
                    identities.extend(keys.into_iter().map(|key| (key, index)));
                }
                continue;
            };
            if let Ok(aggregate_id) = Uuid::parse_str(&target.id) {
                let decision = match basis {
                    LinkBasis::TagName => ResolutionDecision::TagName,
                    LinkBasis::ExternalId | LinkBasis::Recorded => ResolutionDecision::ExternalId,
                };
                self.plan.resolved.push(ResolvedItem {
                    record,
                    item,
                    kind: kind.as_str().to_owned(),
                    aggregate_id,
                    decision,
                });
            }
            self.plan.entities[index].disposition = Disposition::Link { target, basis };
        }
        Ok(())
    }

    /// The record `(record, item)` of `kind` resolves onto by origin in this run's dataset, and whether
    /// the dataset created it.
    async fn by_origin(
        &self,
        kind: MatchableKind,
        record: &str,
        item: Option<&str>,
    ) -> Result<Option<(AggRef, bool)>, AppError> {
        let Some(dataset) = self.dataset() else {
            return Ok(None);
        };
        let store = self.workspace.store();
        let resolved = store
            .resolve_origin(dataset.as_str(), record, item, kind.as_str())
            .await?;
        let Some(resolution) = resolved else {
            return Ok(None);
        };
        let human_id = if kind == MatchableKind::Tag {
            Some(resolution.aggregate_id.clone())
        } else {
            store.human_id_of(kind.as_str(), &resolution.aggregate_id).await?
        };
        Ok(human_id.map(|human_id| {
            let target = AggRef {
                human_id,
                id: resolution.aggregate_id,
            };
            (target, resolution.created)
        }))
    }

    /// The person or family one of `external_ids` names.
    async fn by_external_id(
        &self,
        kind: MatchableKind,
        external_ids: &[vitni_core::text::ExternalId],
    ) -> Result<Option<AggRef>, AppError> {
        let store = self.workspace.store();
        for external_id in external_ids {
            let (authority, value) = (&external_id.authority, &external_id.value);
            let found = if kind == MatchableKind::Family {
                let view = store.find_family_by_external_id(authority, value).await?;
                view.and_then(|view| Some((view.human_id()?.as_str().to_owned(), view.family_id()?.to_string())))
            } else {
                let view = store.find_person_by_external_id(authority, value).await?;
                view.and_then(|view| Some((view.human_id()?.as_str().to_owned(), view.person_id()?.to_string())))
            };
            if let Some((human_id, id)) = found {
                return Ok(Some(AggRef { human_id, id }));
            }
        }
        Ok(None)
    }

    /// Resolves every reference each graph makes: to an entity of the plan, or to a stored record.
    async fn resolve_endpoints(&mut self) -> Result<(), AppError> {
        let mut references = Vec::new();
        for (graph_index, graph) in self.plan.graphs.iter().enumerate() {
            for entity in &graph.entities {
                if let EntityFields::Citation(citation) = &entity.fields {
                    references.push((graph_index, citation.source.clone()));
                }
            }
            for link in &graph.links {
                references.push((graph_index, link.link.owner().reference.clone()));
                for target in link.link.targets() {
                    references.push((graph_index, target.reference.clone()));
                }
            }
        }
        for (graph_index, reference) in references {
            if self.plan.endpoints.contains_key(&(graph_index, reference.clone())) {
                continue;
            }
            let endpoint = self.endpoint_of(graph_index, &reference).await?;
            self.plan.endpoints.insert((graph_index, reference), endpoint);
        }
        Ok(())
    }

    /// Where `reference`, made in `graph`, points; a reference to a duplicate points at the entity it
    /// duplicates.
    async fn endpoint_of(&self, graph: usize, reference: &EntityRef) -> Result<Endpoint, AppError> {
        Ok(match self.named(graph, reference).await? {
            Endpoint::Planned(index) => match self.plan.entities[index].disposition {
                Disposition::Duplicate { of } => Endpoint::Planned(of),
                Disposition::Unchanged { .. }
                | Disposition::Update { .. }
                | Disposition::Link { .. }
                | Disposition::Candidates(_)
                | Disposition::New => Endpoint::Planned(index),
            },
            endpoint @ (Endpoint::Stored { .. } | Endpoint::Dangling) => endpoint,
        })
    }

    async fn named(&self, graph: usize, reference: &EntityRef) -> Result<Endpoint, AppError> {
        Ok(match reference {
            EntityRef::Local(local_id) => self
                .by_local
                .get(&(graph, *local_id))
                .map_or(Endpoint::Dangling, |index| Endpoint::Planned(*index)),
            EntityRef::Origin { kind, record, item } => {
                if let Some(index) = self.by_origin.get(&(*kind, record.clone(), item.clone())) {
                    return Ok(Endpoint::Planned(*index));
                }
                match self.by_origin(*kind, record, item.as_deref()).await? {
                    Some((target, _)) => Endpoint::Stored {
                        kind: *kind,
                        human_id: target.human_id,
                        aggregate_id: target.id,
                    },
                    None => Endpoint::Dangling,
                }
            }
        })
    }

    /// The scope pass: a record resolved onto another dataset's keeps only its identity, and what only
    /// withheld writes reach is withheld too.
    fn scope(&mut self) {
        let mut identity_graphs: Vec<Option<MatchableKind>> = vec![None; self.plan.graphs.len()];
        for entity in &mut self.plan.entities {
            if is_subject(entity, &self.plan.graphs) && is_link(&entity.disposition) {
                entity.scope = WriteScope::Identity;
                identity_graphs[entity.graph] = Some(entity.kind);
            }
        }
        for entity in &mut self.plan.entities {
            if identity_graphs[entity.graph].is_some() && entity.scope == WriteScope::Full {
                entity.scope = WriteScope::Withheld;
            }
        }
        for planned in &mut self.plan.links {
            let Some(subject) = identity_graphs[planned.graph] else {
                continue;
            };
            let link = &self.plan.graphs[planned.graph].links[planned.index].link;
            if subject != MatchableKind::Family || !is_membership(link) {
                planned.scope = WriteScope::Withheld;
            }
        }
        self.withhold_unreached();
    }

    /// Withholds, until nothing changes, every link whose owner or target is withheld, and every entity
    /// some reference reaches when all those references are withheld. A membership link references
    /// its family as well as its member.
    fn withhold_unreached(&mut self) {
        loop {
            let mut changed = false;
            let mut reached: HashMap<usize, bool> = HashMap::new();
            for li in 0..self.plan.links.len() {
                let PlannedLink { graph, index, .. } = self.plan.links[li];
                let link = &self.plan.graphs[graph].links[index].link;
                let owner = self.planned(graph, link.owner().reference);
                let targets: Vec<Option<usize>> = link
                    .targets()
                    .iter()
                    .map(|end| self.planned(graph, end.reference))
                    .collect();
                let withheld_end = owner
                    .into_iter()
                    .chain(targets.iter().copied().flatten())
                    .any(|i| self.plan.entities[i].scope == WriteScope::Withheld);
                if withheld_end && self.plan.links[li].scope != WriteScope::Withheld {
                    self.plan.links[li].scope = WriteScope::Withheld;
                    changed = true;
                }
                let live = self.plan.links[li].scope != WriteScope::Withheld;
                let family = owner.filter(|_| is_membership(link));
                for target in targets.into_iter().flatten().chain(family) {
                    let entry = reached.entry(target).or_insert(false);
                    *entry |= live;
                }
            }
            for (citing, source) in self.citation_sources() {
                let live = self.plan.entities[citing].scope != WriteScope::Withheld;
                let entry = reached.entry(source).or_insert(false);
                *entry |= live;
            }
            for (index, live) in reached {
                let entity = &mut self.plan.entities[index];
                if !live && entity.scope == WriteScope::Full && !is_subject(entity, &self.plan.graphs) {
                    entity.scope = WriteScope::Withheld;
                    changed = true;
                }
            }
            if !changed {
                return;
            }
        }
    }

    /// Each planned citation whose source is a planned entity, with that source.
    fn citation_sources(&self) -> Vec<(usize, usize)> {
        let mut found = Vec::new();
        for (index, planned) in self.plan.entities.iter().enumerate() {
            let Some((_, entity)) = self.plan.staged(index) else {
                continue;
            };
            let EntityFields::Citation(citation) = &entity.fields else {
                continue;
            };
            if let Some(source) = self.planned(planned.graph, &citation.source) {
                found.push((index, source));
            }
        }
        found
    }

    /// The plan index `reference`, made in `graph`, names, if it names a planned entity.
    fn planned(&self, graph: usize, reference: &EntityRef) -> Option<usize> {
        match self.plan.endpoint(graph, reference) {
            Endpoint::Planned(index) => Some(*index),
            Endpoint::Stored { .. } | Endpoint::Dangling => None,
        }
    }

    /// The dry-run pass: tells an unchanged record from an updated one, and a link already on record
    /// from one to write.
    async fn dry_run(&mut self) -> Result<(), AppError> {
        let dry_run = Arc::new(DryRun::default());
        let session = self.session.clone().with_dry_run(Arc::clone(&dry_run));
        let template = Provenance::default();
        let dataset = self.dataset().cloned();
        let run = self.session.import_run().map(|run| run.id());
        let file_asserted_at = self.plan.file_asserted_at;
        let writer = Writer {
            workspace: self.workspace,
            session: &session,
            template: &template,
            run: dataset.as_ref().zip(run),
            file_asserted_at,
        };
        let ids: Vec<Option<String>> = self
            .plan
            .entities
            .iter()
            .map(|entity| entity.disposition.target().map(|target| target.human_id.clone()))
            .collect();
        let mut fields: BTreeMap<usize, Vec<String>> = BTreeMap::new();
        self.dry_run_entities(&writer, &dry_run, &mut fields).await;
        self.dry_run_links(&writer, &dry_run, &ids, &mut fields).await;
        for (index, mut written) in fields {
            written.sort();
            written.dedup();
            let planned = &mut self.plan.entities[index];
            if let Disposition::Unchanged { target } = &planned.disposition {
                planned.disposition = Disposition::Update {
                    target: target.clone(),
                    fields: written,
                };
            }
        }
        Ok(())
    }

    /// Dry-runs the writes of every record this dataset made, collecting what each would write.
    async fn dry_run_entities(&self, writer: &Writer<'_>, dry_run: &DryRun, fields: &mut BTreeMap<usize, Vec<String>>) {
        for index in 0..self.plan.entities.len() {
            let planned = &self.plan.entities[index];
            let (Disposition::Unchanged { target }, WriteScope::Full) = (&planned.disposition, planned.scope) else {
                continue;
            };
            let Some((graph, entity)) = self.plan.staged(index) else {
                continue;
            };
            let outcome = writer.update(&graph.record, entity, &target.human_id).await;
            let mut written: Vec<String> = dry_run
                .take()
                .into_iter()
                .map(|write| field_name(write.field))
                .collect();
            if outcome.is_err() {
                written.push("error".to_owned());
            }
            if !written.is_empty() {
                fields.entry(index).or_default().extend(written);
            }
        }
    }

    /// Dry-runs every link whose ends all exist, marking those with nothing to write and collecting what
    /// the rest would write onto their owners.
    async fn dry_run_links(
        &mut self,
        writer: &Writer<'_>,
        dry_run: &DryRun,
        ids: &[Option<String>],
        fields: &mut BTreeMap<usize, Vec<String>>,
    ) {
        for li in 0..self.plan.links.len() {
            let PlannedLink {
                graph, index, scope, ..
            } = self.plan.links[li];
            if scope == WriteScope::Withheld {
                continue;
            }
            let staged = &self.plan.graphs[graph].links[index];
            let resolver = Resolver {
                plan: &self.plan,
                graph,
                ids,
            };
            let ends = std::iter::once(staged.link.owner().reference)
                .chain(staged.link.targets().into_iter().map(|end| end.reference))
                .map(|reference| self.plan.endpoint(graph, reference).clone())
                .collect::<Vec<_>>();
            if ends.contains(&Endpoint::Dangling) {
                self.plan.links[li].dangling = true;
                self.plan.links[li].writes = false;
                continue;
            }
            let all_exist = ends.iter().all(|end| match end {
                Endpoint::Planned(i) => ids[*i].is_some(),
                Endpoint::Stored { .. } => true,
                Endpoint::Dangling => false,
            });
            if !all_exist {
                continue;
            }
            let record = self.plan.graphs[graph].record.clone();
            let outcome = writer
                .link(&record, staged.item.as_deref(), &staged.link, &resolver)
                .await;
            let mut written: Vec<String> = dry_run
                .take()
                .into_iter()
                .map(|write| field_name(write.field))
                .collect();
            match outcome {
                Ok(()) => {}
                Err(LinkError::Dangling) => {
                    self.plan.links[li].dangling = true;
                    self.plan.links[li].writes = false;
                    continue;
                }
                Err(LinkError::App(_)) => written.push("error".to_owned()),
            }
            self.plan.links[li].writes = !written.is_empty();
            if let Some(owner) = self.planned(graph, staged.link.owner().reference)
                && !written.is_empty()
            {
                fields.entry(owner).or_default().extend(written);
            }
        }
    }

    /// The candidate pass: matches each new person, place, source and repository against the workspace.
    async fn match_candidates(&mut self) -> Result<(), AppError> {
        let wanted = |entity: &PlannedEntity| {
            entity.scope == WriteScope::Full
                && entity.disposition == Disposition::New
                && candidates::MATCHED.contains(&entity.kind)
        };
        if !self.plan.entities.iter().any(wanted) {
            return Ok(());
        }
        let run = self.session.import_run().map(|run| run.id());
        let found = candidates::find(self.workspace, &self.plan, self.dataset().zip(run)).await?;
        for (index, similar) in found {
            if !similar.is_empty() {
                self.plan.entities[index].disposition = Disposition::Candidates(similar);
            }
        }
        Ok(())
    }
}

/// The least band a record is proposed at as a candidate.
pub(crate) const CANDIDATE_BAND: MatchBand = MatchBand::Possible;

/// The field a dry write asserts, or `record` for one asserting none.
fn field_name(field: Option<String>) -> String {
    field.unwrap_or_else(|| "record".to_owned())
}

/// Whether `kind` can be a graph's own record whose resolution scopes the graph.
fn subject_kind(kind: MatchableKind) -> bool {
    kind == MatchableKind::Person || kind == MatchableKind::Family
}

/// Whether `link` makes a family's membership: written for a family resolved onto another dataset's,
/// never for a person resolved onto another dataset's.
fn is_membership(link: &LinkKind) -> bool {
    match link {
        LinkKind::Partner { .. } | LinkKind::Child { .. } => true,
        LinkKind::Participation { .. }
        | LinkKind::FamilyEvent { .. }
        | LinkKind::EventPlace { .. }
        | LinkKind::Enclosure { .. }
        | LinkKind::CitationOf { .. }
        | LinkKind::MediaOf { .. }
        | LinkKind::NoteOf { .. }
        | LinkKind::TagOf { .. }
        | LinkKind::SourceRepository { .. }
        | LinkKind::Association { .. } => false,
    }
}

/// Whether `disposition` resolved onto a record this dataset did not make.
fn is_link(disposition: &Disposition) -> bool {
    match disposition {
        Disposition::Link { .. } | Disposition::Duplicate { .. } => true,
        Disposition::Unchanged { .. } | Disposition::Update { .. } | Disposition::Candidates(_) | Disposition::New => {
            false
        }
    }
}

/// Whether `entity` is its graph's own person or family.
fn is_subject(entity: &PlannedEntity, graphs: &[RecordGraph]) -> bool {
    subject_kind(entity.kind)
        && graphs.get(entity.graph).is_some_and(|graph| {
            graph
                .entities
                .iter()
                .any(|staged| staged.local_id == entity.local_id && staged.item.is_none())
        })
}

/// What makes two entities of one import the same record: a person's or family's external ids, a
/// tag's folded name.
fn identity_keys(fields: &EntityFields) -> Vec<String> {
    let external = |kind: &str, ids: &[vitni_core::text::ExternalId]| -> Vec<String> {
        ids.iter()
            .map(|id| format!("{kind}\u{1f}{}\u{1f}{}", id.authority, id.value))
            .collect()
    };
    match fields {
        EntityFields::Person(person) => external("person", &person.external_ids),
        EntityFields::Family(family) => external("family", &family.external_ids),
        EntityFields::Tag(tag) => vec![format!("tag\u{1f}{}", fold(&tag.name))],
        EntityFields::Event(_)
        | EntityFields::Place(_)
        | EntityFields::Source(_)
        | EntityFields::Citation(_)
        | EntityFields::Media(_)
        | EntityFields::Note(_)
        | EntityFields::Repository(_) => Vec::new(),
    }
}

/// A tag name as identity compares it.
fn fold(name: &str) -> String {
    name.trim().to_lowercase()
}

/// Every tag of the workspace, by its folded name.
async fn tag_names(workspace: &Workspace) -> Result<HashMap<String, String>, AppError> {
    let mut names = HashMap::new();
    for tag in crate::tag::list_tags(workspace).await? {
        if let Some(name) = &tag.name {
            names.entry(fold(name)).or_insert(tag.id);
        }
    }
    Ok(names)
}
