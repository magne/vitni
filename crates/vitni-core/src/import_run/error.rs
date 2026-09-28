//! The `ImportRun` domain-error taxonomy.

use thiserror::Error;

use crate::ids::ImportRunId;

/// A reason the `ImportRun` aggregate refused a command.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ImportRunError {
    /// The command targets a run that was never started.
    #[error("import run {0} does not exist")]
    NotFound(ImportRunId),
    /// `StartImportRun` was issued for a run that already exists.
    #[error("import run {0} already exists")]
    AlreadyExists(ImportRunId),
    /// The run has already finished or been abandoned.
    #[error("import run {0} has already ended")]
    AlreadyEnded(ImportRunId),
}
