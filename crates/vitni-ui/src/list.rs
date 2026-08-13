//! Generic list vocabulary: the framework-neutral shapes the master-detail list framework is driven
//! by (ADR 0008). A [`RowVm`] is one entity rendered as a list row — already-localized strings so a
//! renderer stays dumb — and [`visible_rows`] applies a [`ListQuery`] (search + sort) purely, so the
//! filtering/sorting is unit-testable without a renderer.

/// One entity as a row in a list view: an already-localized title and optional subtitle/avatar,
/// keyed by its stable user-facing id.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowVm {
    /// The stable navigation key (e.g. `I0001`, or a tag's UUID). Also the trailing id label unless
    /// [`id_label`](Self::id_label) overrides it.
    pub id: String,
    /// The primary, already-localized title (e.g. a display name).
    pub title: String,
    /// An optional secondary line (dates, place, …), already localized.
    pub subtitle: Option<String>,
    /// An optional short avatar text (e.g. initials). Ignored when [`dot_color`](Self::dot_color) is
    /// set (the row shows a colour dot instead).
    pub avatar: Option<String>,
    /// An optional colour for a leading dot avatar (a CSS string), for rows whose identity is a
    /// colour rather than initials (tags). When set, the dot replaces `avatar`.
    pub dot_color: Option<String>,
    /// An optional trailing id label distinct from the navigation `id` (e.g. a tag's `#hex` colour,
    /// since its `id` is an internal UUID never shown). Falls back to `id` when `None`.
    pub id_label: Option<String>,
}

impl RowVm {
    /// The trailing id label the row shows: [`id_label`](Self::id_label) when set, else the `id`.
    #[must_use]
    pub fn display_id(&self) -> &str {
        self.id_label.as_deref().unwrap_or(&self.id)
    }

    /// Whether this row matches `needle` (case-insensitive) in its title, subtitle, or displayed id.
    #[must_use]
    pub fn matches(&self, needle: &str) -> bool {
        let needle = needle.to_lowercase();
        self.title.to_lowercase().contains(&needle)
            || self
                .subtitle
                .as_deref()
                .is_some_and(|subtitle| subtitle.to_lowercase().contains(&needle))
            || self.display_id().to_lowercase().contains(&needle)
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

impl RowSort {
    /// The next order in the toolbar sort-button cycle (id ↑ → id ↓ → name ↑ → name ↓ → id ↑).
    #[must_use]
    pub fn next(self) -> Self {
        match self {
            Self::IdAsc => Self::IdDesc,
            Self::IdDesc => Self::TitleAsc,
            Self::TitleAsc => Self::TitleDesc,
            Self::TitleDesc => Self::IdAsc,
        }
    }
}

/// The live list state: the search query and the sort order.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListQuery {
    /// The current search text; empty means "no filter".
    pub query: String,
    /// The current sort order.
    pub sort: RowSort,
}

/// The row `delta` steps from the currently-selected one within the visible (filtered + sorted)
/// order — the `[`/`]` prev/next-record navigation over a master-detail list.
///
/// `selected` is the current selection's id, if any. Movement clamps at both ends (no wrap). With no
/// selection (or a selection filtered out of view), a forward step (`delta >= 0`) lands on the first
/// visible row and a backward step on the last. Returns `None` only when nothing is visible.
#[must_use]
pub fn step_row(rows: &[RowVm], q: &ListQuery, selected: Option<&str>, delta: isize) -> Option<RowVm> {
    let visible = visible_rows(rows, q);
    let last = visible.len().checked_sub(1)?;
    let next = match selected.and_then(|id| visible.iter().position(|row| row.id == id)) {
        Some(current) => {
            let current = isize::try_from(current).unwrap_or(0);
            let last_index = isize::try_from(last).unwrap_or(0);
            usize::try_from((current + delta).clamp(0, last_index)).unwrap_or(0)
        }
        None if delta >= 0 => 0,
        None => last,
    };
    visible.get(next).cloned()
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
    use super::{ListQuery, RowSort, RowVm, step_row, visible_rows};

    fn rows() -> Vec<RowVm> {
        vec![
            RowVm {
                id: "I0002".to_owned(),
                title: "Ada Lovelace".to_owned(),
                subtitle: Some("female".to_owned()),
                avatar: Some("AL".to_owned()),
                ..RowVm::default()
            },
            RowVm {
                id: "I0001".to_owned(),
                title: "Charles Babbage".to_owned(),
                subtitle: None,
                avatar: Some("CB".to_owned()),
                ..RowVm::default()
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

    #[test]
    fn step_row_moves_within_the_sorted_order() {
        let q = ListQuery::default(); // IdAsc: I0001 (Charles) then I0002 (Ada)
        assert_eq!(
            step_row(&rows(), &q, Some("I0001"), 1).map(|r| r.id),
            Some("I0002".to_owned())
        );
        assert_eq!(
            step_row(&rows(), &q, Some("I0002"), -1).map(|r| r.id),
            Some("I0001".to_owned())
        );
    }

    #[test]
    fn step_row_clamps_at_both_ends() {
        let q = ListQuery::default();
        assert_eq!(
            step_row(&rows(), &q, Some("I0001"), -1).map(|r| r.id),
            Some("I0001".to_owned())
        );
        assert_eq!(
            step_row(&rows(), &q, Some("I0002"), 1).map(|r| r.id),
            Some("I0002".to_owned())
        );
    }

    #[test]
    fn step_row_without_selection_lands_on_the_first_or_last() {
        let q = ListQuery::default();
        assert_eq!(step_row(&rows(), &q, None, 1).map(|r| r.id), Some("I0001".to_owned()));
        assert_eq!(step_row(&rows(), &q, None, -1).map(|r| r.id), Some("I0002".to_owned()));
    }

    #[test]
    fn step_row_honours_the_active_filter() {
        let q = ListQuery {
            query: "ada".to_owned(),
            sort: RowSort::IdAsc,
        };
        // Only Ada is visible; stepping stays on her (and a filtered-out selection falls back to first).
        assert_eq!(
            step_row(&rows(), &q, Some("I0001"), 1).map(|r| r.id),
            Some("I0002".to_owned())
        );
        assert_eq!(
            step_row(&rows(), &q, Some("I0002"), 1).map(|r| r.id),
            Some("I0002".to_owned())
        );
    }

    #[test]
    fn step_row_on_empty_is_none() {
        assert!(step_row(&[], &ListQuery::default(), None, 1).is_none());
    }
}
