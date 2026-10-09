//! Reviewing a plan before it is committed (ADR 0040 §3, §4): each staged entity with candidates is
//! put to the user one pair at a time, best candidate first, and the plan is committed with their
//! answers.
//!
//! - *Same* on a person imports the record as its own persona and merges it into the candidate after
//!   the commit, so the record's evidence stays a unit (ADR 0039).
//! - *Same* on a place, source or repository reuses the candidate: the import is planned again with the
//!   decision, which resolves the entity onto it, records the resolution on the run, and reassesses the
//!   rest of the plan with it as evidence. The commit adds the fields the candidate lacks.
//! - *Not the same* moves on to the next candidate; once the entity is written, it is distinguished
//!   from each candidate rejected. An entity with every candidate rejected is simply new.
//! - *Decide later* imports it as new with the pairs left for the review queue.
//!
//! A bulk import adds two explicit bulk answers (ADR 0040 §3): *Same* for every pair of the current
//! kind whose candidate is probable, each recorded with the assessment of its own pair, and *Decide the
//! rest later*.
//!
//! Merges and distinctions are the user's decisions, written as the run's human operator; the import's
//! own writes stay the importer's.

use vitni_core::ids::ImportRunId;
use vitni_core::matching::{MatchAssessment, MatchBand, MatchableKind};
use vitni_core::origin::{DatasetId, RecordOrigin};
use vitni_core::provenance::Timestamp;

use crate::dto::AggRef;
use crate::error::AppError;
use crate::identity::IdentityDecision;
use crate::session::Session;
use crate::similar::SimilarRecord;
use crate::staging::commit::{CommitControl, CommitFailure, CommitOutcome, commit_import};
use crate::staging::graph::RecordGraph;
use crate::staging::plan::{
    DecidedMatch, Disposition, ImportPlan, PlanError, PlanSummary, PlannedEntity, plan_decided, staged_label,
};
use crate::use_case::Provenance;
use crate::workspace::Workspace;

/// One pair put to the user: a staged entity and one stored record the engine judged possibly the same.
#[derive(Debug, Clone, PartialEq)]
pub struct MatchQuestion {
    /// The kind of both records.
    pub kind: MatchableKind,
    /// The staged entity's display label (a person's name, a place's name).
    pub incoming_label: String,
    /// The dataset record the staged entity comes from, when the import writes a run.
    pub incoming_origin: Option<RecordOrigin>,
    /// The stored record.
    pub candidate: AggRef,
    /// The stored record's display label, or its human id when it has none.
    pub candidate_label: String,
    /// The dataset record an import created the stored record from, if any.
    pub candidate_origin: Option<RecordOrigin>,
    /// The engine's assessment of the pair, the stored record its left side — the side a *Same*
    /// keeps.
    pub assessment: MatchAssessment,
    /// Which of the plan's entities with candidates this is, from 1.
    pub position: usize,
    /// How many of the plan's entities have candidates.
    pub total: usize,
    /// The pairs a bulk *Same* would decide with this one, when its candidate is probable.
    pub group: Option<MatchGroup>,
}

/// The still-open pairs of one kind whose candidate is in one band: what *Treat all as the same*
/// decides.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MatchGroup {
    /// The least band every pair's candidate is in.
    pub band: MatchBand,
    /// How many entities have such a pair, the current one included.
    pub remaining: usize,
}

/// The least band a bulk *Same* is offered at.
const GROUP_BAND: MatchBand = MatchBand::Probable;

/// The user's answer to a [`MatchQuestion`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PairAnswer {
    /// The staged entity is the stored record.
    Same(IdentityDecision),
    /// It is not; the next candidate is asked about.
    Distinct(IdentityDecision),
    /// Import it as new and leave its candidates for the review queue.
    Later,
}

/// What the user answers to a [`MatchQuestion`] in an assisted import: the pair's answer, or leaving
/// the record or the session.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum MatchReply {
    /// An answer about the pair.
    Pair(Box<PairAnswer>),
    /// Import nothing of this record.
    Skip,
    /// End the import session, importing nothing of this record.
    Cancel,
}

/// What the user answers when an assisted import shows what a record adds to the records already in the
/// tree before it is written (ADR 0046).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanReply {
    /// Write the record.
    Import,
    /// Import nothing of this record.
    Skip,
    /// End the import session, importing nothing of this record.
    Cancel,
}

/// What the user does with a bulk import's plan once it is shown (ADR 0040 §4).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PlanStep {
    /// Commit it, asking about each possible match first.
    Review,
    /// Commit it, leaving every possible match for later.
    DeferMatches,
    /// Write nothing.
    Discard,
}

/// What the user answers to a [`MatchQuestion`] in a bulk import: the pair's answer, a bulk answer
/// (ADR 0040 §3), or ending the import.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReviewReply {
    /// An answer about the pair.
    Pair(Box<PairAnswer>),
    /// *Same* for every open pair of the question's [`MatchGroup`], with this decision's provenance.
    SameForGroup(Box<IdentityDecision>),
    /// *Decide later* for this and every remaining pair.
    DeferRest,
    /// Write nothing.
    Cancel,
}

/// An import plan under review.
#[derive(Debug, Clone)]
pub struct ImportReview {
    plan: ImportPlan,
    file_asserted_at: Option<Timestamp>,
    /// The dataset and run the import writes, if it writes one.
    run: Option<(DatasetId, ImportRunId)>,
    decided: Vec<DecidedMatch>,
    /// The answers given about each entity, by its index in the plan.
    answers: Vec<Answers>,
}

/// What the user answered about one entity.
#[derive(Debug, Clone, Default)]
struct Answers {
    /// Each candidate it is not, with the decision.
    rejected: Vec<(AggRef, IdentityDecision)>,
    /// The answer that ended its questions, if one did.
    settled: Option<Settled>,
}

/// An answer that ends an entity's questions.
#[derive(Debug, Clone)]
enum Settled {
    /// A person that is the stored record: imported, then merged into it.
    Merge(Box<(AggRef, IdentityDecision)>),
    /// A record the plan now resolves onto the stored one.
    Reused,
    /// Imported as new, its candidates left for later.
    Later,
}

impl ImportReview {
    /// Plans `graphs` for review, as [`plan_import`](crate::plan_import) does.
    ///
    /// # Errors
    ///
    /// As [`plan_import`](crate::plan_import).
    pub async fn plan(
        workspace: &Workspace,
        session: &Session,
        graphs: Vec<RecordGraph>,
        file_asserted_at: Option<Timestamp>,
    ) -> Result<Self, PlanError> {
        let plan = plan_decided(workspace, session, graphs, file_asserted_at, &[]).await?;
        let answers = vec![Answers::default(); plan.entities.len()];
        Ok(Self {
            plan,
            file_asserted_at,
            run: session.import_run().map(|run| (run.dataset().clone(), run.id())),
            decided: Vec::new(),
            answers,
        })
    }

    /// The plan's counts by kind and the records it changes, as the answers so far leave it: a pair
    /// answered *Same* or rejected counts as the new record the commit writes for it.
    #[must_use]
    pub fn summary(&self) -> PlanSummary {
        let mut entities = self.plan.entities.clone();
        settle(&self.answers, &mut entities);
        self.plan.summary_of(&entities)
    }

    /// The plan as the answers so far leave it.
    #[must_use]
    pub fn plan_so_far(&self) -> &ImportPlan {
        &self.plan
    }

    /// The next pair to put to the user, or `None` when every entity with candidates is answered for.
    ///
    /// # Errors
    ///
    /// An [`AppError`] if the stored record cannot be read.
    pub async fn next_question(&self, workspace: &Workspace) -> Result<Option<MatchQuestion>, AppError> {
        let Some((index, similar)) = self.pending() else {
            return Ok(None);
        };
        let entity = &self.plan.entities[index];
        let (position, total) = self.position(index);
        let incoming = self.plan.staged(index);
        let incoming_label = incoming
            .map(|(_, staged)| staged_label(&staged.fields))
            .unwrap_or_default();
        let incoming_origin = incoming
            .zip(self.run.clone())
            .map(|((graph, staged), (dataset, run))| RecordOrigin {
                dataset,
                record: graph.record.clone(),
                item: staged.item.clone(),
                digest: None,
                run,
            });
        let candidate = similar.record.clone();
        let candidate_label = stored_label(workspace, entity.kind, &candidate.human_id).await?;
        let candidate_origin = crate::record_origin(workspace, entity.kind, &candidate.human_id).await?;
        let group = (similar.assessment.band >= GROUP_BAND).then(|| MatchGroup {
            band: GROUP_BAND,
            remaining: self.group(entity.kind).len(),
        });
        Ok(Some(MatchQuestion {
            kind: entity.kind,
            incoming_label,
            incoming_origin,
            candidate,
            candidate_label,
            candidate_origin,
            assessment: similar.assessment.clone().mirrored(),
            position,
            total,
            group,
        }))
    }

    /// Records `answer` to the current question ([`next_question`](Self::next_question)); nothing
    /// happens when there is none.
    ///
    /// # Errors
    ///
    /// As [`plan_import`](crate::plan_import), when a *Same* on a place, source or repository plans the
    /// import again.
    pub async fn answer(
        &mut self,
        workspace: &Workspace,
        session: &Session,
        answer: PairAnswer,
    ) -> Result<(), PlanError> {
        let Some((index, similar)) = self.pending() else {
            return Ok(());
        };
        let target = similar.record.clone();
        let kind = self.plan.entities[index].kind;
        match answer {
            PairAnswer::Distinct(decision) => self.answers[index].rejected.push((target, decision)),
            PairAnswer::Later => self.answers[index].settled = Some(Settled::Later),
            PairAnswer::Same(decision) if kind == MatchableKind::Person => {
                self.answers[index].settled = Some(Settled::Merge(Box::new((target, decision))));
            }
            PairAnswer::Same(_) => {
                let Some((graph, staged)) = self.plan.staged(index) else {
                    return Ok(());
                };
                self.decided.push(DecidedMatch {
                    kind,
                    record: graph.record.clone(),
                    item: staged.item.clone(),
                    target,
                });
                self.answers[index].settled = Some(Settled::Reused);
                let graphs = self.plan.graphs.clone();
                self.plan = plan_decided(workspace, session, graphs, self.file_asserted_at, &self.decided).await?;
            }
        }
        Ok(())
    }

    /// Answers *Same* for every open pair of the current question's kind whose candidate is probable —
    /// the current one included — each with `decision`'s provenance and the assessment of its own pair.
    /// Nothing is answered when the current question has no group (its candidate is not probable).
    /// A *Same* on a place, source or repository plans the import again once, with every such decision.
    ///
    /// # Errors
    ///
    /// As [`plan_import`](crate::plan_import), when the import is planned again.
    pub async fn answer_group(
        &mut self,
        workspace: &Workspace,
        session: &Session,
        decision: IdentityDecision,
    ) -> Result<(), PlanError> {
        let Some((index, current)) = self.pending() else {
            return Ok(());
        };
        if current.assessment.band < GROUP_BAND {
            return Ok(());
        }
        let kind = self.plan.entities[index].kind;
        let mut replan = false;
        for (index, similar) in self.group(kind) {
            let target = similar.record.clone();
            let decision = IdentityDecision {
                assessment: Some(similar.assessment.clone().mirrored().evidence()),
                ..decision.clone()
            };
            if kind == MatchableKind::Person {
                self.answers[index].settled = Some(Settled::Merge(Box::new((target, decision))));
                continue;
            }
            let Some((graph, staged)) = self.plan.staged(index) else {
                continue;
            };
            self.decided.push(DecidedMatch {
                kind,
                record: graph.record.clone(),
                item: staged.item.clone(),
                target,
            });
            self.answers[index].settled = Some(Settled::Reused);
            replan = true;
        }
        if replan {
            let graphs = self.plan.graphs.clone();
            self.plan = plan_decided(workspace, session, graphs, self.file_asserted_at, &self.decided).await?;
        }
        Ok(())
    }

    /// Answers *Decide later* for every entity still with a pair to ask about.
    pub fn defer_rest(&mut self) {
        while let Some((index, _)) = self.pending() {
            self.answers[index].settled = Some(Settled::Later);
        }
    }

    /// Commits the plan as `session`, then writes the user's merges and distinctions as `operator`.
    /// `control` follows the plan's writes and may stop them, as in
    /// [`commit_import`](crate::commit_import); the decisions are still written for every entity the
    /// commit wrote before it stopped.
    ///
    /// # Errors
    ///
    /// A [`CommitFailure`] carrying the first failed write and what was written before it.
    pub async fn commit(
        mut self,
        workspace: &Workspace,
        session: &Session,
        operator: &Session,
        template: &Provenance,
        control: &mut dyn CommitControl,
    ) -> Result<CommitOutcome, CommitFailure> {
        settle(&self.answers, &mut self.plan.entities);
        let outcome = commit_import(workspace, session, &self.plan, template, control).await?;
        match self.decide(workspace, operator, &outcome).await {
            Ok(()) => Ok(outcome),
            Err(error) => Err(CommitFailure {
                outcome: Box::new(outcome),
                error,
            }),
        }
    }

    /// Writes the distinctions and merges the user decided, now that each entity has a record.
    async fn decide(&self, workspace: &Workspace, operator: &Session, outcome: &CommitOutcome) -> Result<(), AppError> {
        for (index, answers) in self.answers.iter().enumerate() {
            let entity = &self.plan.entities[index];
            let Some(imported) = outcome
                .entities
                .iter()
                .find(|committed| committed.graph == entity.graph && committed.local_id == entity.local_id)
            else {
                continue;
            };
            if let Some(Settled::Reused) = answers.settled {
                continue;
            }
            for (target, decision) in &answers.rejected {
                distinguish(
                    workspace,
                    operator,
                    entity.kind,
                    &imported.human_id,
                    &target.human_id,
                    decision,
                )
                .await?;
            }
            if let Some(Settled::Merge(merge)) = &answers.settled {
                let (target, decision) = merge.as_ref();
                crate::merge_persons(
                    workspace,
                    operator,
                    &target.human_id,
                    &imported.human_id,
                    decision.clone(),
                )
                .await?;
            }
        }
        Ok(())
    }

    /// The first entity with a candidate still to ask about, and that candidate.
    fn pending(&self) -> Option<(usize, &SimilarRecord)> {
        (0..self.plan.entities.len()).find_map(|index| self.next_candidate(index).map(|next| (index, next)))
    }

    /// Every entity of `kind` whose next candidate to ask about is at least in the group band, with it.
    fn group(&self, kind: MatchableKind) -> Vec<(usize, SimilarRecord)> {
        let mut group = Vec::new();
        for (index, entity) in self.plan.entities.iter().enumerate() {
            if entity.kind != kind {
                continue;
            }
            if let Some(next) = self.next_candidate(index)
                && next.assessment.band >= GROUP_BAND
            {
                group.push((index, next.clone()));
            }
        }
        group
    }

    /// Entity `index`'s next candidate to ask about: its best one not yet rejected, unless an answer
    /// settled it.
    fn next_candidate(&self, index: usize) -> Option<&SimilarRecord> {
        let Disposition::Candidates(similar) = &self.plan.entities.get(index)?.disposition else {
            return None;
        };
        let answers = &self.answers[index];
        if answers.settled.is_some() {
            return None;
        }
        similar.iter().find(|candidate| {
            !answers
                .rejected
                .iter()
                .any(|(rejected, _)| rejected.id == candidate.record.id)
        })
    }

    /// Where entity `index` stands among the plan's entities with candidates: its position from 1, and
    /// their number. An entity a *Same* reused still counts, though the plan now resolves it, so the
    /// positions after it do not step back.
    fn position(&self, index: usize) -> (usize, usize) {
        let mut position = 0;
        let mut total = 0;
        for (i, entity) in self.plan.entities.iter().enumerate() {
            let reused = match self.answers[i].settled {
                Some(Settled::Reused) => true,
                Some(Settled::Merge(_) | Settled::Later) | None => false,
            };
            if reused || has_candidates(&entity.disposition) {
                total += 1;
                if i <= index {
                    position += 1;
                }
            }
        }
        (position, total)
    }
}

/// Whether `disposition` holds candidates.
fn has_candidates(disposition: &Disposition) -> bool {
    match disposition {
        Disposition::Candidates(_) => true,
        Disposition::Unchanged { .. }
        | Disposition::Update { .. }
        | Disposition::Link { .. }
        | Disposition::Duplicate { .. }
        | Disposition::New => false,
    }
}

/// Records that the imported record `imported` of `kind` is not `stored`, the stored record first —
/// the left side of the assessment the decision carries.
async fn distinguish(
    workspace: &Workspace,
    operator: &Session,
    kind: MatchableKind,
    imported: &str,
    stored: &str,
    decision: &IdentityDecision,
) -> Result<(), AppError> {
    let decision = decision.clone();
    match kind {
        MatchableKind::Person => crate::distinguish_persons(workspace, operator, stored, imported, decision).await,
        MatchableKind::Place => crate::distinguish_places(workspace, operator, stored, imported, decision).await,
        MatchableKind::Source => crate::distinguish_sources(workspace, operator, stored, imported, decision).await,
        MatchableKind::Repository => {
            crate::distinguish_repositories(workspace, operator, stored, imported, decision).await
        }
        MatchableKind::Family
        | MatchableKind::Event
        | MatchableKind::Citation
        | MatchableKind::Media
        | MatchableKind::Note
        | MatchableKind::Tag => {
            tracing::warn!(
                kind = kind.as_str(),
                "no candidates are proposed for this kind; nothing distinguished"
            );
            Ok(())
        }
    }
}

/// Gives each entity whose candidates are answered for the disposition the commit writes it with.
fn settle(answers: &[Answers], entities: &mut [PlannedEntity]) {
    for (index, answers) in answers.iter().enumerate() {
        let entity = &mut entities[index];
        let decided = match answers.settled {
            Some(Settled::Merge(..)) => true,
            Some(Settled::Later | Settled::Reused) => false,
            None => !answers.rejected.is_empty(),
        };
        if decided && has_candidates(&entity.disposition) {
            entity.disposition = Disposition::New;
        }
    }
}

/// A stored record's display label, or `human_id` when it has none.
async fn stored_label(workspace: &Workspace, kind: MatchableKind, human_id: &str) -> Result<String, AppError> {
    let label = crate::history::record_label(workspace, kind.as_str(), human_id).await?;
    Ok(label.unwrap_or_else(|| human_id.to_owned()))
}
