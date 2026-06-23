//! Generic list vocabulary: the framework-neutral shapes the master-detail list framework is driven
//! by (ADR 0008). A [`RowVm`] is one entity rendered as a list row — already-localized strings so a
//! renderer stays dumb — and [`visible_rows`] applies a [`ListQuery`] (search + sort) purely, so the
//! filtering/sorting is unit-testable without a renderer.

/// One entity as a row in a list view: an already-localized title and optional subtitle/avatar,
/// keyed by its stable user-facing id.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RowVm {
    /// The stable key and trailing id label (e.g. `I0001`).
    pub id: String,
    /// The primary, already-localized title (e.g. a display name).
    pub title: String,
    /// An optional secondary line (dates, place, …), already localized.
    pub subtitle: Option<String>,
    /// An optional short avatar text (e.g. initials).
    pub avatar: Option<String>,
}

impl RowVm {
    /// Whether this row matches `needle` (case-insensitive) in its title, subtitle, or id.
    #[must_use]
    pub fn matches(&self, needle: &str) -> bool {
        let needle = needle.to_lowercase();
        self.title.to_lowercase().contains(&needle)
            || self
                .subtitle
                .as_deref()
                .is_some_and(|subtitle| subtitle.to_lowercase().contains(&needle))
            || self.id.to_lowercase().contains(&needle)
    }
}

/// How a list is ordered.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum RowSort {
    /// By id, ascending (the default; lists come from the app ordered by id).
    #[default]
    IdAsc,
    /// By id, descending.
    IdDesc,
    /// By title, ascending (case-insensitive).
    TitleAsc,
    /// By title, descending (case-insensitive).
    TitleDesc,
}

/// The live list state: the search query and the sort order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListQuery {
    /// The current search text; empty means "no filter".
    pub query: String,
    /// The current sort order.
    pub sort: RowSort,
}

/// Filters `rows` by the query (when non-empty) and sorts them per `q.sort`.
#[must_use]
pub fn visible_rows(rows: &[RowVm], q: &ListQuery) -> Vec<RowVm> {
    let mut visible: Vec<RowVm> = if q.query.is_empty() {
        rows.to_vec()
    } else {
        rows.iter().filter(|row| row.matches(&q.query)).cloned().collect()
    };
    match q.sort {
        RowSort::IdAsc => visible.sort_by(|a, b| a.id.cmp(&b.id)),
        RowSort::IdDesc => visible.sort_by(|a, b| b.id.cmp(&a.id)),
        RowSort::TitleAsc => visible.sort_by_key(|row| row.title.to_lowercase()),
        RowSort::TitleDesc => visible.sort_by_key(|row| std::cmp::Reverse(row.title.to_lowercase())),
    }
    visible
}

#[cfg(test)]
mod tests {
    use super::{ListQuery, RowSort, RowVm, visible_rows};

    fn rows() -> Vec<RowVm> {
        vec![
            RowVm {
                id: "I0002".to_owned(),
                title: "Ada Lovelace".to_owned(),
                subtitle: Some("female".to_owned()),
                avatar: Some("AL".to_owned()),
            },
            RowVm {
                id: "I0001".to_owned(),
                title: "Charles Babbage".to_owned(),
                subtitle: None,
                avatar: Some("CB".to_owned()),
            },
        ]
    }

    #[test]
    fn empty_query_returns_all_rows() {
        let visible = visible_rows(&rows(), &ListQuery::default());
        assert_eq!(visible.len(), 2);
    }

    #[test]
    fn query_matches_case_insensitively_on_title() {
        let q = ListQuery {
            query: "ada".to_owned(),
            sort: RowSort::IdAsc,
        };
        let visible = visible_rows(&rows(), &q);
        assert_eq!(visible.len(), 1);
        assert_eq!(visible[0].id, "I0002");
    }

    #[test]
    fn query_matches_on_subtitle_and_id() {
        let by_subtitle = visible_rows(
            &rows(),
            &ListQuery {
                query: "FEMALE".to_owned(),
                sort: RowSort::IdAsc,
            },
        );
        assert_eq!(by_subtitle.len(), 1);
        let by_id = visible_rows(
            &rows(),
            &ListQuery {
                query: "i0001".to_owned(),
                sort: RowSort::IdAsc,
            },
        );
        assert_eq!(by_id.len(), 1);
        assert_eq!(by_id[0].id, "I0001");
    }

    #[test]
    fn sort_orders_apply() {
        let id_asc = visible_rows(&rows(), &ListQuery::default());
        assert_eq!([id_asc[0].id.as_str(), id_asc[1].id.as_str()], ["I0001", "I0002"]);
        let id_desc = visible_rows(
            &rows(),
            &ListQuery {
                query: String::new(),
                sort: RowSort::IdDesc,
            },
        );
        assert_eq!([id_desc[0].id.as_str(), id_desc[1].id.as_str()], ["I0002", "I0001"]);
        let title_asc = visible_rows(
            &rows(),
            &ListQuery {
                query: String::new(),
                sort: RowSort::TitleAsc,
            },
        );
        assert_eq!(title_asc[0].title, "Ada Lovelace");
        let title_desc = visible_rows(
            &rows(),
            &ListQuery {
                query: String::new(),
                sort: RowSort::TitleDesc,
            },
        );
        assert_eq!(title_desc[0].title, "Charles Babbage");
    }

    #[test]
    fn empty_list_stays_empty() {
        assert!(visible_rows(&[], &ListQuery::default()).is_empty());
    }
}
