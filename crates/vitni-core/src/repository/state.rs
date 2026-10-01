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
    /// The assertion that set the current `restrictions`, so retracting it clears them (the set is
    /// replaced wholesale, not accumulated, so it cannot be attributed per-element — ADR 0021 §3).
    #[serde(default)]
    pub restrictions_assertion: Option<AssertionId>,
    /// The repositories merged into this survivor (ADR 0039 §1), each attributed to the `RepositoriesMerged`
    /// assertion that recorded it, so undoing that assertion removes the link.
    #[serde(default)]
    pub merged: Vec<Attributed<RepositoryId>>,
    /// The repositories concluded to be different from this one, each attributed to the
    /// `RepositoriesDistinguished` assertion that recorded it, so undoing that assertion lifts it.
    #[serde(default)]
    pub distinguished: Vec<Attributed<RepositoryId>>,
    /// Assertion ids that are currently live (not retracted/superseded), so corrections can be
    /// validated (data-model §10.1).
    pub live_assertions: BTreeSet<AssertionId>,
}

impl RepositoryState {
    /// Whether this repository holds a live identity decision — merged or distinguished — about `other`.
    #[must_use]
    pub(crate) fn has_decided(&self, other: RepositoryId) -> bool {
        self.merged.iter().chain(&self.distinguished).any(|d| d.value == other)
    }

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
        if self.restrictions_assertion == Some(target) {
            self.restrictions.clear();
            self.restrictions_assertion = None;
        }
        self.merged.retain(|m| m.assertion_id != target);
        self.distinguished.retain(|d| d.assertion_id != target);
        self.live_assertions.remove(&target);
    }
}
