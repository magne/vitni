//! Repository events — the past-tense assertions the aggregate produces (data-model §10).

use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::address::Address;
use crate::assertions::{Envelope, EventBody};
use crate::enums::{RepositoryType, Restriction};
use crate::ids::{AssertionId, HumanId, NoteId, RepositoryId, TagId};
use crate::matching::MatchEvidence;
use crate::text::Url;

/// A single Repository assertion plus its provenance envelope (ADR 0004 §1).
pub type RepositoryEvent = Envelope<RepositoryEventBody>;

/// The Repository claim variants (data-model §10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, strum::VariantNames)]
#[serde(tag = "type")]
pub enum RepositoryEventBody {
    /// A repository aggregate was created.
    RepositoryCreated {
        /// The created repository.
        repository_id: RepositoryId,
        /// The user-facing identifier.
        human_id: HumanId,
    },
    /// The repository's type was set / changed.
    RepositoryTypeSet {
        /// The repository.
        repository_id: RepositoryId,
        /// The new repository type.
        repository_type: RepositoryType,
    },
    /// The repository's name was set / changed.
    NameSet {
        /// The repository.
        repository_id: RepositoryId,
        /// The new name.
        name: String,
    },
    /// A postal address was added.
    AddressAdded {
        /// The repository.
        repository_id: RepositoryId,
        /// The address.
        address: Address,
    },
    /// A URL was added.
    UrlAdded {
        /// The repository.
        repository_id: RepositoryId,
        /// The URL.
        url: Url,
    },
    /// A note was attached to the repository.
    NoteAttached {
        /// The repository.
        repository_id: RepositoryId,
        /// The attached note.
        note_id: NoteId,
    },
    /// A tag was applied to the repository.
    Tagged {
        /// The repository.
        repository_id: RepositoryId,
        /// The applied tag.
        tag_id: TagId,
    },
    /// A tag was removed from the repository.
    Untagged {
        /// The repository.
        repository_id: RepositoryId,
        /// The removed tag.
        tag_id: TagId,
    },
    /// The repository's privacy restrictions were set / changed (GEDCOM `RESN` — data-model §6).
    RestrictionsChanged {
        /// The repository.
        repository_id: RepositoryId,
        /// The new restriction set (empty = unrestricted).
        restrictions: BTreeSet<Restriction>,
    },
    /// A prior assertion was retracted (non-destructive correction — data-model §10).
    AssertionRetracted {
        /// The repository.
        repository_id: RepositoryId,
        /// The assertion being retracted.
        target: AssertionId,
    },
    /// A prior assertion was superseded; the replacement event accompanies this one.
    AssertionSuperseded {
        /// The repository.
        repository_id: RepositoryId,
        /// The assertion being superseded.
        target: AssertionId,
    },
    /// The repository's user-facing identifier was changed (data-model §7).
    HumanIdChanged {
        /// The repository.
        repository_id: RepositoryId,
        /// The new user-facing identifier.
        human_id: HumanId,
        /// The identifier in effect before this change (for the audit trail).
        old_human_id: HumanId,
    },
    /// Another repository record was concluded to be this one (ADR 0039 §1): a same-as link on the survivor
    /// that leaves the merged record's own stream untouched (data-model §9).
    RepositoriesMerged {
        /// The repository that survives as the cluster's record.
        surviving: RepositoryId,
        /// The repository merged into it.
        merged: RepositoryId,
        /// The matching engine's assessment the user decided on (ADR 0039 §2); `None` for a merge made
        /// without the engine.
        assessment: Option<MatchEvidence>,
    },
    /// Another repository record was concluded to be a different repository from this one (ADR 0039 §1), so the
    /// pair is never proposed as a duplicate again.
    RepositoriesDistinguished {
        /// The repository the decision is recorded on.
        repository: RepositoryId,
        /// The repository it is distinct from.
        other: RepositoryId,
        /// The matching engine's assessment the user decided on (ADR 0039 §2); `None` for a decision
        /// made without the engine.
        assessment: Option<MatchEvidence>,
    },
}

impl EventBody for RepositoryEventBody {
    fn type_name(&self) -> &'static str {
        match self {
            Self::RepositoryCreated { .. } => "RepositoryCreated",
            Self::RepositoryTypeSet { .. } => "RepositoryTypeSet",
            Self::NameSet { .. } => "NameSet",
            Self::AddressAdded { .. } => "AddressAdded",
            Self::UrlAdded { .. } => "UrlAdded",
            Self::NoteAttached { .. } => "NoteAttached",
            Self::Tagged { .. } => "Tagged",
            Self::Untagged { .. } => "Untagged",
            Self::RestrictionsChanged { .. } => "RestrictionsChanged",
            Self::AssertionRetracted { .. } => "AssertionRetracted",
            Self::AssertionSuperseded { .. } => "AssertionSuperseded",
            Self::HumanIdChanged { .. } => "HumanIdChanged",
            Self::RepositoriesMerged { .. } => "RepositoriesMerged",
            Self::RepositoriesDistinguished { .. } => "RepositoriesDistinguished",
        }
    }

    fn version(&self) -> &'static str {
        "1.0"
    }
}
