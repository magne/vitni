//! [`RepositoryView`] — the conclusion-layer read model for a Repository (data-model §6).
//!
//! Rebuilt by folding the same events as the aggregate (ADR 0009).

use cqrs_es::{EventEnvelope, View};
use serde::{Deserialize, Serialize};

use std::collections::BTreeSet;

use crate::address::Address;
use crate::assertions::Attributed;
use crate::enums::{RepositoryType, Restriction};
use crate::ids::{HumanId, NoteId, RepositoryId, TagId};
use crate::repository::decide::evolve;
use crate::repository::state::RepositoryState;
use crate::text::Url;

/// The current best synthesis of a Repository, derived from the event log (data-model §6).
#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RepositoryView {
    state: RepositoryState,
}

impl RepositoryView {
    /// Returns `true` once the repository has been created.
    #[must_use]
    pub fn exists(&self) -> bool {
        self.state.exists
    }

    /// The repository's id, once created.
    #[must_use]
    pub fn repository_id(&self) -> Option<RepositoryId> {
        self.state.repository_id
    }

    /// The user-facing identifier.
    #[must_use]
    pub fn human_id(&self) -> Option<&HumanId> {
        self.state.human_id.as_ref()
    }

    /// The repository's type, if set.
    #[must_use]
    pub fn repository_type(&self) -> Option<&RepositoryType> {
        self.state.repository_type.as_ref().map(|t| &t.value)
    }

    /// The repository's name, if set.
    #[must_use]
    pub fn name(&self) -> Option<&str> {
        self.state.name.as_ref().map(|n| n.value.as_str())
    }

    /// All currently-live addresses, in assertion order.
    #[must_use]
    pub fn addresses(&self) -> Vec<&Address> {
        self.state.addresses.iter().map(|a| &a.value).collect()
    }

    /// All currently-live URLs, in assertion order.
    #[must_use]
    pub fn urls(&self) -> Vec<&Url> {
        self.state.urls.iter().map(|u| &u.value).collect()
    }

    /// All currently-live attached notes, in assertion order.
    #[must_use]
    pub fn notes(&self) -> Vec<NoteId> {
        self.state.notes.iter().map(|n| n.value).collect()
    }

    /// All currently-applied tags, in assertion order.
    #[must_use]
    pub fn tags(&self) -> Vec<TagId> {
        self.state.tags.iter().map(|t| t.value).collect()
    }

    /// The repository's privacy restrictions (GEDCOM `RESN`).
    #[must_use]
    pub fn restrictions(&self) -> &BTreeSet<Restriction> {
        &self.state.restrictions
    }

    /// Currently-live URLs, each paired with the `AssertionId` that introduced it — the read side of
    /// the per-row correction (Edit supersedes it, Remove retracts it).
    #[must_use]
    pub fn urls_with_assertions(&self) -> &[Attributed<Url>] {
        &self.state.urls
    }

    /// Currently-live attached notes, each paired with the attach `AssertionId` (the detach target).
    #[must_use]
    pub fn notes_with_assertions(&self) -> &[Attributed<NoteId>] {
        &self.state.notes
    }
}

impl View<RepositoryState> for RepositoryView {
    fn update(&mut self, event: &EventEnvelope<RepositoryState>) {
        evolve(&mut self.state, &event.payload);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::assertions::Attributed;
    use crate::ids::AssertionId;
    use uuid::Uuid;

    #[test]
    fn notes_with_assertions_exposes_the_attach_assertion() {
        let aid = AssertionId::from_uuid(Uuid::from_u128(7));
        let note = crate::ids::NoteId::from_uuid(Uuid::from_u128(8));
        let state = RepositoryState {
            notes: vec![Attributed {
                assertion_id: aid,
                value: note,
            }],
            ..Default::default()
        };
        let view = RepositoryView { state };
        assert_eq!(view.notes_with_assertions()[0].assertion_id, aid);
    }
}
