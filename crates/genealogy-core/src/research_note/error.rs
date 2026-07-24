//! The `ResearchNote` domain-error taxonomy (ADR 0028, data-model §10.1).

use thiserror::Error;

use crate::ids::{AssertionId, ResearchNoteId};

/// A reason the `ResearchNote` aggregate refused a command (ADR 0028, data-model §10.1).
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ResearchNoteError {
    /// The command targets a research note that does not exist.
    #[error("research note {0} does not exist")]
    NotFound(ResearchNoteId),
    /// `CreateResearchNote` was issued for a research note that already exists.
    #[error("research note {0} already exists")]
    AlreadyExists(ResearchNoteId),
    /// `CreateResearchNote`/`AddSubject` named a subject the projection does not know (the §9
    /// aggregate-tax check) — a dangling Person/Family/Event/Place reference.
    #[error("the research note's subject does not exist")]
    UnknownSubject,
    /// `CreateResearchNote` was given an empty subject set, or `RemoveSubject` targeted the note's
    /// last remaining subject — a `ResearchNote` always names at least one subject (ADR 0028 §2).
    #[error("a research note must name at least one subject")]
    SubjectRequired,
    /// `SetBody` supplied an all-blank argument text.
    #[error("a research note's body must not be empty")]
    EmptyBody,
    /// `RetractAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    RetractsMissingAssertion(AssertionId),
    /// `SupersedeAssertion` referenced an assertion that is unknown or already retracted.
    #[error("assertion {0} is not present or already retracted")]
    SupersedesMissingAssertion(AssertionId),
}
