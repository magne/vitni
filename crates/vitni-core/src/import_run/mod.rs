//! The `ImportRun` aggregate (ADR 0037 §5).
//!
//! One import of one file (or one assisted session) by one operator: the answer to "who ran this
//! import, over what, when, and how did it end". Its operator is the invoking human, while every
//! assertion the run writes keeps the importer's `Software` agent and names the run in its
//! [`RecordOrigin`](crate::origin::RecordOrigin). A run carries no `HumanId` and no assertion chain:
//! it starts once and ends once, finished or abandoned. Datasets are a projection over runs.

mod aggregate;
pub mod command;
pub mod decide;
pub mod error;
pub mod event;
pub mod state;
pub mod view;

pub use command::{ImportRunCommand, ImportRunCommandEnvelope, NewImportRun};
pub use decide::{decide, evolve};
pub use error::ImportRunError;
pub use event::{AbandonReason, ImportCounts, ImportRunEvent, ImportRunEventBody, ResolutionDecision, ResolvedItem};
pub use state::{ImportRunState, ImportRunStatus};
pub use view::ImportRunView;
