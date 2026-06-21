//! Repository commands — imperative operator intent (data-model §10).

use crate::address::Address;
use crate::enums::RepositoryType;
use crate::ids::{AssertionId, HumanId, NoteId, RepositoryId, TagId};
use crate::provenance::AssertionMeta;
use crate::text::Url;

/// Operator intent against a Repository aggregate (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RepositoryCommand {
    /// Create a new repository.
    CreateRepository {
        /// The application-generated id for the new repository.
        repository_id: RepositoryId,
        /// The user-facing identifier.
        human_id: HumanId,
    },
    /// Set (or change) the repository's type.
    SetRepositoryType {
        /// The target repository.
        repository_id: RepositoryId,
        /// The new repository type.
        repository_type: RepositoryType,
    },
    /// Set (or change) the repository's name.
    SetName {
        /// The target repository.
        repository_id: RepositoryId,
        /// The repository name.
        name: String,
    },
    /// Add a postal address for the repository.
    AddAddress {
        /// The target repository.
        repository_id: RepositoryId,
        /// The address.
        address: Address,
    },
    /// Add a URL for the repository.
    AddUrl {
        /// The target repository.
        repository_id: RepositoryId,
        /// The URL.
        url: Url,
    },
    /// Attach a note to the repository.
    AttachNote {
        /// The target repository.
        repository_id: RepositoryId,
        /// The note to attach.
        note_id: NoteId,
    },
    /// Apply a tag to the repository.
    Tag {
        /// The target repository.
        repository_id: RepositoryId,
        /// The tag to apply.
        tag_id: TagId,
    },
    /// Remove a tag from the repository.
    Untag {
        /// The target repository.
        repository_id: RepositoryId,
        /// The tag to remove.
        tag_id: TagId,
    },
    /// Retract a prior assertion (non-destructive).
    RetractAssertion {
        /// The target repository.
        repository_id: RepositoryId,
        /// The assertion to retract.
        target: AssertionId,
    },
    /// Supersede a prior assertion with a replacement command.
    SupersedeAssertion {
        /// The target repository.
        repository_id: RepositoryId,
        /// The assertion to supersede.
        target: AssertionId,
        /// The command producing the replacement assertion.
        replacement: Box<RepositoryCommand>,
    },
}

/// A command paired with its supplied non-deterministic inputs (ADR 0004 §3).
///
/// This is the `cqrs-es` `Aggregate::Command` for the Repository aggregate.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RepositoryCommandEnvelope {
    /// The pre-generated assertion id and provenance context.
    pub meta: AssertionMeta,
    /// The operator's intent.
    pub command: RepositoryCommand,
}
