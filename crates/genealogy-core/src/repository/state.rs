//! [`RepositoryState`] — the folded aggregate state used by the decision core.
//!
//! Type and name are last-writer-wins; addresses, URLs, notes, and tags accumulate. Each is
//! attributed to the [`AssertionId`] that introduced it, so a correction can remove exactly the
//! right entry.

use std::collections::BTreeSet;

use serde::{Deserialize, Serialize};

use crate::address::Address;
use crate::assertions::Attributed;
use crate::enums::{RepositoryType, Restriction};
use crate::ids::{AssertionId, HumanId, NoteId, RepositoryId, TagId};
use crate::text::Url;

/// The folded state of a Repository aggregate (data-model §6).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryState {
    /// Whether `RepositoryCreated` has been seen.
    pub exists: bool,
    /// The repository's id (set on creation).
    pub repository_id: Option<RepositoryId>,
    /// The user-facing identifier.
    pub human_id: Option<HumanId>,
    /// The repository's type (last writer wins).
    pub repository_type: Option<Attributed<RepositoryType>>,
    /// The repository's name (last writer wins).
    pub name: Option<Attributed<String>>,
    /// All currently-live addresses, in assertion order.
    pub addresses: Vec<Attributed<Address>>,
    /// All currently-live URLs, in assertion order.
    pub urls: Vec<Attributed<Url>>,
    /// All currently-live attached notes, in assertion order.
    pub notes: Vec<Attributed<NoteId>>,
    /// All currently-applied tags, in assertion order.
    pub tags: Vec<Attributed<TagId>>,
    /// The repository's privacy restrictions (GEDCOM `RESN`, last writer wins — data-model §6).
    pub restrictions: BTreeSet<Restriction>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl RepositoryState {
    /// Removes every value introduced by `target` and drops it from the live set.
    pub(crate) fn remove_assertion(&mut self, target: AssertionId) {
        self.addresses.retain(|a| a.assertion_id != target);
        self.urls.retain(|u| u.assertion_id != target);
        self.notes.retain(|n| n.assertion_id != target);
        self.tags.retain(|t| t.assertion_id != target);
        if self.repository_type.as_ref().is_some_and(|t| t.assertion_id == target) {
            self.repository_type = None;
        }
        if self.name.as_ref().is_some_and(|n| n.assertion_id == target) {
            self.name = None;
        }
        self.live_assertions.remove(&target);
    }
}
