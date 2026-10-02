//! Staged import (ADR 0040): an importer submits record graphs, [`plan_import`] decides what each
//! entity is before anything is written, and [`commit_import`] writes the plan.
//!
//! The importer resolves nothing. The plan resolves every entity the one way the host knows — by
//! origin, by external id, a tag by its name — and matches the rest against the workspace, so a
//! re-import of an unchanged file plans every entity as unchanged and writes nothing. The commit is
//! resumable: everything it wrote is keyed by origin, so an interrupted commit finishes when the same
//! import is planned and committed again.

mod candidates;
mod commit;
mod graph;
mod plan;
mod review;
mod write;

pub use commit::{CommitControl, CommitFailure, CommitOutcome, CommittedEntity, RunToEnd, commit_import};
pub use graph::{
    EntityFields, EntityRef, GraphError, LinkKind, LocalId, RecordGraph, StagedCitation, StagedEntity, StagedEvent,
    StagedFamily, StagedLink, StagedMedia, StagedNote, StagedPerson, StagedPlace, StagedRepository, StagedSource,
    StagedTag,
};
pub use plan::{
    DecidedMatch, Disposition, ImportPlan, KindCounts, LinkBasis, PlanCounts, PlanError, PlanSummary, PlannedEntity,
    PlannedLink, WriteScope, plan_import,
};
pub use review::{ImportReview, MatchGroup, MatchQuestion, MatchReply, PairAnswer, PlanStep, ReviewReply};
