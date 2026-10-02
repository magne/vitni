//! Committing a plan (ADR 0040 §5): the planned writes, in dependency order, each stamped with its
//! entity's or link's origin and the run.
//!
//! Places, sources, repositories, tags, media and notes are written first, then citations, persons,
//! families and events, then the links between them. Each new aggregate is created by one command with
//! its external ids and origin, so it commits whole. Across aggregates the commit is sequenced, not
//! atomic: an interrupted commit leaves what it wrote, every write keyed by its origin, so planning the
//! same import again resolves all of it as unchanged and the commit finishes the rest.

use std::collections::BTreeMap;

use vitni_core::import_run::ResolvedItem;
use vitni_core::matching::MatchableKind;

use crate::error::AppError;
use crate::session::Session;
use crate::staging::graph::LocalId;
use crate::staging::plan::{Disposition, ImportPlan, Resolver, WriteScope};
use crate::staging::write::{LinkError, Writer};
use crate::use_case::Provenance;
use crate::workspace::Workspace;

/// The order kinds are created in, so a citation finds its source.
const KIND_ORDER: [MatchableKind; 10] = [
    MatchableKind::Place,
    MatchableKind::Source,
    MatchableKind::Repository,
    MatchableKind::Tag,
    MatchableKind::Media,
    MatchableKind::Note,
    MatchableKind::Citation,
    MatchableKind::Person,
    MatchableKind::Family,
    MatchableKind::Event,
];

/// Lets the caller follow a commit, and stop it between two writes.
pub trait CommitControl: Send {
    /// Called before each write, `done` of `total` written so far. `false` stops the commit there.
    fn proceed(&mut self, done: u32, total: u32) -> bool;
}

/// A [`CommitControl`] that never stops.
#[derive(Debug, Clone, Copy, Default)]
pub struct RunToEnd;

impl CommitControl for RunToEnd {
    fn proceed(&mut self, _done: u32, _total: u32) -> bool {
        true
    }
}

/// An entity the commit wrote or resolved, and its record.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommittedEntity {
    /// The index of its graph in [`ImportPlan::graphs`].
    pub graph: usize,
    /// Its local id in that graph.
    pub local_id: LocalId,
    /// The record's human id (a tag's id).
    pub human_id: String,
}

/// What a commit wrote.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct CommitOutcome {
    /// Every entity with a record, in plan order.
    pub entities: Vec<CommittedEntity>,
    /// Aggregates created, by kind (`Aggregate::TYPE`).
    pub created: BTreeMap<String, u32>,
    /// The items resolved onto existing records.
    pub resolved: Vec<ResolvedItem>,
    /// Entities with candidates, created as new with the decision left for later.
    pub deferred: u32,
    /// Links, and citations, left out because a reference named nothing.
    pub dangling: u32,
    /// Whether the control stopped the commit before its end.
    pub interrupted: bool,
}

/// A commit that failed part-way, with what it wrote before the failure.
#[derive(Debug, thiserror::Error)]
#[error("{error}")]
pub struct CommitFailure {
    /// What was written.
    pub outcome: Box<CommitOutcome>,
    /// The failure.
    #[source]
    pub error: AppError,
}

/// Writes `plan` into `workspace` as `session`, every write carrying `template`'s confidence and the
/// origin of its entity or link in the session's import run, if any. `control` is asked before each
/// write and may stop the commit; an interrupted commit is finished by planning and committing the same
/// import again.
///
/// # Errors
///
/// A [`CommitFailure`] carrying the first write's error and what was written before it.
pub async fn commit_import(
    workspace: &Workspace,
    session: &Session,
    plan: &ImportPlan,
    template: &Provenance,
    control: &mut dyn CommitControl,
) -> Result<CommitOutcome, CommitFailure> {
    let run = session.import_run();
    let writer = Writer {
        workspace,
        session,
        template,
        run: run.map(|run| (run.dataset(), run.id())),
        file_asserted_at: plan.file_asserted_at,
    };
    let mut commit = Commit {
        plan,
        writer,
        ids: plan
            .entities
            .iter()
            .map(|entity| entity.disposition.target().map(|target| target.human_id.clone()))
            .collect(),
        outcome: CommitOutcome {
            resolved: plan.resolved.clone(),
            ..CommitOutcome::default()
        },
        done: 0,
        total: 0,
    };
    let entities = commit.entity_order();
    let links = commit.link_order();
    commit.total = u32::try_from(entities.len() + links.len()).unwrap_or(u32::MAX);
    match commit.run(control, &entities, &links).await {
        Ok(()) => Ok(commit.finish()),
        Err(error) => Err(CommitFailure {
            outcome: Box::new(commit.finish()),
            error,
        }),
    }
}

/// A commit in progress.
struct Commit<'a> {
    plan: &'a ImportPlan,
    writer: Writer<'a>,
    /// Each entity's human id, once it has a record.
    ids: Vec<Option<String>>,
    outcome: CommitOutcome,
    done: u32,
    total: u32,
}

impl Commit<'_> {
    /// The entities with something to write, in creation order.
    fn entity_order(&self) -> Vec<usize> {
        let mut order = Vec::new();
        for kind in KIND_ORDER {
            for (index, entity) in self.plan.entities.iter().enumerate() {
                let writes = match (&entity.disposition, entity.scope) {
                    (_, WriteScope::Withheld)
                    | (Disposition::Unchanged { .. } | Disposition::Link { .. }, WriteScope::Full) => false,
                    (
                        Disposition::New
                        | Disposition::Candidates(_)
                        | Disposition::Update { .. }
                        | Disposition::Duplicate { .. },
                        WriteScope::Full,
                    )
                    | (_, WriteScope::Identity) => true,
                };
                if entity.kind == kind && writes {
                    order.push(index);
                }
            }
        }
        order
    }

    /// The links with something to write, in plan order.
    fn link_order(&self) -> Vec<usize> {
        let mut order = Vec::new();
        for (index, link) in self.plan.links.iter().enumerate() {
            if link.dangling {
                continue;
            }
            if link.scope != WriteScope::Withheld && link.writes {
                order.push(index);
            }
        }
        order
    }

    async fn run(
        &mut self,
        control: &mut dyn CommitControl,
        entities: &[usize],
        links: &[usize],
    ) -> Result<(), AppError> {
        for &index in entities {
            if !self.proceed(control) {
                return Ok(());
            }
            self.entity(index).await?;
        }
        for &index in links {
            if !self.proceed(control) {
                return Ok(());
            }
            self.link(index).await?;
        }
        Ok(())
    }

    fn proceed(&mut self, control: &mut dyn CommitControl) -> bool {
        if !control.proceed(self.done, self.total) {
            self.outcome.interrupted = true;
            return false;
        }
        self.done += 1;
        true
    }

    async fn entity(&mut self, index: usize) -> Result<(), AppError> {
        let planned = &self.plan.entities[index];
        let Some((graph, entity)) = self.plan.staged(index) else {
            return Ok(());
        };
        let record = graph.record.as_str();
        match (&planned.disposition, planned.scope) {
            (Disposition::Duplicate { of }, scope) => {
                // The entity it duplicates is of the same kind and earlier, so already written.
                self.ids[index] = self.ids.get(*of).cloned().flatten();
                if let (Some(human_id), WriteScope::Identity) = (&self.ids[index], scope) {
                    self.writer.identity(record, entity, human_id).await?;
                }
            }
            (Disposition::Update { target, .. }, WriteScope::Full) => {
                self.writer.update(record, entity, &target.human_id).await?;
            }
            (Disposition::New | Disposition::Candidates(_), WriteScope::Full) => {
                let resolver = Resolver {
                    plan: self.plan,
                    graph: planned.graph,
                    ids: &self.ids,
                };
                let Some(human_id) = self.writer.create(record, entity, &resolver).await? else {
                    self.outcome.dangling += 1;
                    return Ok(());
                };
                self.ids[index] = Some(human_id);
                *self
                    .outcome
                    .created
                    .entry(planned.kind.as_str().to_owned())
                    .or_insert(0) += 1;
                if let Disposition::Candidates(_) = planned.disposition {
                    self.outcome.deferred += 1;
                }
            }
            (disposition, WriteScope::Identity) => {
                if let Some(target) = disposition.target() {
                    self.writer.identity(record, entity, &target.human_id).await?;
                }
            }
            (Disposition::Unchanged { .. } | Disposition::Link { .. }, WriteScope::Full)
            | (_, WriteScope::Withheld) => {}
        }
        Ok(())
    }

    async fn link(&mut self, index: usize) -> Result<(), AppError> {
        let planned = &self.plan.links[index];
        let graph = &self.plan.graphs[planned.graph];
        let staged = &graph.links[planned.index];
        let resolver = Resolver {
            plan: self.plan,
            graph: planned.graph,
            ids: &self.ids,
        };
        match self
            .writer
            .link(&graph.record, staged.item.as_deref(), &staged.link, &resolver)
            .await
        {
            Ok(()) => Ok(()),
            Err(LinkError::Dangling) => {
                self.outcome.dangling += 1;
                Ok(())
            }
            Err(LinkError::App(error)) => Err(error),
        }
    }

    /// The outcome, with every entity that has a record.
    fn finish(mut self) -> CommitOutcome {
        for (index, entity) in self.plan.entities.iter().enumerate() {
            let Some(human_id) = self.ids[index].clone() else {
                continue;
            };
            self.outcome.entities.push(CommittedEntity {
                graph: entity.graph,
                local_id: entity.local_id,
                human_id,
            });
        }
        self.outcome.dangling +=
            u32::try_from(self.plan.links.iter().filter(|link| link.dangling).count()).unwrap_or(0);
        self.outcome
    }
}
