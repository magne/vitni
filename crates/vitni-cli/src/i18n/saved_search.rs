//! Localized text for [`SavedSearchError`] (ADR 0052 §8).

use i18n_embed_fl::fl;
use vitni_app::{ClauseGap, SavedSearchError, StaleClause, UnreadableSavedSearch};

use super::Localizer;

impl Localizer {
    /// `Saved Olsens`.
    #[must_use]
    pub fn saved_search_saved(&self, name: &str) -> String {
        fl!(self.loader, "saved-search-saved", name = name)
    }

    /// `Deleted Olsens`.
    #[must_use]
    pub fn saved_search_deleted(&self, name: &str) -> String {
        fl!(self.loader, "saved-search-deleted", name = name)
    }

    /// What `saved-search list` prints when there is nothing to list.
    #[must_use]
    pub fn saved_search_list_empty(&self) -> String {
        fl!(self.loader, "saved-search-list-empty")
    }

    /// One saved search: its name and the kind it lists.
    #[must_use]
    pub fn saved_search_line(&self, name: &str, kind: &str) -> String {
        fl!(self.loader, "saved-search-line", name = name, kind = kind)
    }

    /// One entry this version cannot read, by its position in `[[saved_searches]]`.
    #[must_use]
    pub fn saved_search_unreadable_line(&self, entry: &UnreadableSavedSearch) -> String {
        let Some(name) = &entry.name else {
            return fl!(
                self.loader,
                "saved-search-unreadable-line-nameless",
                position = entry.position
            );
        };
        fl!(
            self.loader,
            "saved-search-unreadable-line",
            position = entry.position,
            name = name.clone()
        )
    }

    pub(super) fn saved_search_error(&self, error: &SavedSearchError) -> String {
        match error {
            SavedSearchError::EmptyName => fl!(self.loader, "err-saved-search-empty-name"),
            SavedSearchError::DuplicateName { name } => {
                fl!(self.loader, "err-saved-search-duplicate-name", name = name.clone())
            }
            SavedSearchError::NotFound { reference } => {
                fl!(self.loader, "err-saved-search-not-found", reference = reference.clone())
            }
            SavedSearchError::Removed => fl!(self.loader, "err-saved-search-removed"),
            SavedSearchError::Broken { name, clauses } => {
                let mut text = fl!(self.loader, "err-saved-search-broken", name = name.clone());
                for clause in clauses {
                    text.push('\n');
                    text.push_str(&self.broken_clause_line(clause));
                }
                text
            }
        }
    }

    fn broken_clause_line(&self, clause: &StaleClause) -> String {
        let position = clause.index + 1;
        let Some(gap) = clause.gap() else {
            return fl!(
                self.loader,
                "err-saved-search-broken-clause",
                position = position,
                clause = clause.named(":"),
                reason = self.list_query_error(&clause.error)
            );
        };
        match gap {
            ClauseGap::Both => {
                fl!(
                    self.loader,
                    "err-saved-search-broken-clause-nameless",
                    position = position
                )
            }
            ClauseGap::Column => fl!(
                self.loader,
                "err-saved-search-broken-clause-no-column",
                position = position,
                clause = clause.named(":")
            ),
            ClauseGap::Operator => fl!(
                self.loader,
                "err-saved-search-broken-clause-no-operator",
                position = position,
                clause = clause.named(":")
            ),
        }
    }
}
