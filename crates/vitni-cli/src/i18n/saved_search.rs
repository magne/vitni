//! Localized text for [`SavedSearchError`] (ADR 0052 §8).

use i18n_embed_fl::fl;
use vitni_app::{ClauseGap, SavedSearchError, StaleClause};

use super::Localizer;

impl Localizer {
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
