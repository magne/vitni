//! The record picker: framework-neutral find-or-create state for a link to another record
//! (`docs/phase5/record-editing.html` §6b, `edit-patterns.html` §c).
//!
//! A picker turns a free-text `human_id` input into a search-and-select control: the operator types,
//! [`picker_rows`] filters the already-loaded [`RowVm`]s (reusing [`RowVm::matches`]), and picking a
//! row records a [`PickerSelection`] (its `human_id` + display title). The renderer
//! (`genealogy-ui-dioxus`) draws it; this module holds only the state and the pure filtering, so it is
//! unit-testable without a framework. Options load once per open form via the existing `list_*`
//! use-cases ([`list_intent`] maps a [`Category`] to its list [`Intent`]); a server-side `search_*`
//! with a `LIMIT` is a flagged follow-up.

use crate::list::RowVm;
use crate::navigation::{Category, Intent};

/// The most rows a picker shows at once; the operator narrows with the query rather than scrolling a
/// long floating list.
pub const PICKER_MAX_ROWS: usize = 6;

/// A record the operator picked: its stable user-facing id and the already-localized display title.
///
/// Held UI-locally; a draft copies the `human_id` into its [`RecordLink`](crate::view_model::RecordLink)
/// when the pick lands, so no `*Edit`/request shape changes to carry a title.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PickerSelection {
    /// The picked record's stable user-facing id (e.g. `I0042`).
    pub human_id: String,
    /// The picked record's already-localized display title (e.g. a person's name).
    pub title: String,
}

impl PickerSelection {
    /// Builds a selection from a list row (its navigation id + title).
    #[must_use]
    pub fn from_row(row: &RowVm) -> Self {
        Self {
            human_id: row.id.clone(),
            title: row.title.clone(),
        }
    }

    /// The collapsed display for the selection: `Title (I0042)`, or the bare id when the title is just
    /// the id (an untitled record).
    #[must_use]
    pub fn display(&self) -> String {
        if self.title == self.human_id {
            self.human_id.clone()
        } else {
            format!("{} ({})", self.title, self.human_id)
        }
    }
}

/// The live state of one picker control: the search query, whether the result list is open, and the
/// current selection (if any).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PickerState {
    /// The current search text; empty shows all options (capped at [`PICKER_MAX_ROWS`]).
    pub query: String,
    /// Whether the result list is open. Closes on pick/clear/Esc, on an outside click, or on focus
    /// leaving the control (the renderer floats the list and dismisses it like a native dropdown).
    pub open: bool,
    /// The picked record, or `None` when nothing is selected yet.
    pub selection: Option<PickerSelection>,
}

impl PickerState {
    /// Records `row` as the selection, clearing the query and closing the list.
    pub fn pick(&mut self, row: &RowVm) {
        self.selection = Some(PickerSelection::from_row(row));
        self.query.clear();
        self.open = false;
    }

    /// Clears the selection and query, closing the list.
    pub fn clear(&mut self) {
        self.selection = None;
        self.query.clear();
        self.open = false;
    }
}

/// A keyboard move against a picker's navigable index (the matched rows plus, when shown, the
/// trailing "+ New …" row). Passed to [`next_active`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ActiveMove {
    /// Move to the previous item, clamping at the first (no wrap).
    Up,
    /// Move to the next item, clamping at the last (no wrap).
    Down,
    /// Jump to the first item.
    First,
    /// Jump to the last item.
    Last,
}

/// The next highlighted index for `mv` over `len` navigable items: clamps into `[0, len - 1]` like a
/// native `<select>` (arrow keys never wrap past either end) and returns `0` when `len == 0` (nothing
/// to highlight). `current` is assumed already in range; an out-of-range `current` is clamped by the
/// same rules.
#[must_use]
pub fn next_active(current: usize, mv: ActiveMove, len: usize) -> usize {
    let Some(last) = len.checked_sub(1) else {
        return 0;
    };
    match mv {
        ActiveMove::Up => current.min(last).saturating_sub(1),
        ActiveMove::Down => (current + 1).min(last),
        ActiveMove::First => 0,
        ActiveMove::Last => last,
    }
}

/// The rows a picker shows for `query`: the `options` matching the query (via [`RowVm::matches`], all
/// of them when the query is empty), minus any id in `exclude`, capped at [`PICKER_MAX_ROWS`].
#[must_use]
pub fn picker_rows(options: &[RowVm], query: &str, exclude: &[String]) -> Vec<RowVm> {
    let mut rows: Vec<RowVm> = Vec::new();
    for row in options {
        if exclude.iter().any(|id| id == &row.id) {
            continue;
        }
        if !query.is_empty() && !row.matches(query) {
            continue;
        }
        rows.push(row.clone());
        if rows.len() >= PICKER_MAX_ROWS {
            break;
        }
    }
    rows
}

/// The [`Intent`] that loads a category's list, so a picker can populate its options — for every
/// pickable category. Returns `None` for [`Category::Dashboard`] (not a record list) and
/// [`Category::Tags`] (tags are picked by name, never by id — data-model §9).
#[must_use]
pub fn list_intent(category: Category) -> Option<Intent> {
    match category {
        Category::Dashboard | Category::Tags => None,
        Category::People => Some(Intent::ShowList),
        Category::Families => Some(Intent::ShowFamilyList),
        Category::Events => Some(Intent::ShowEventList),
        Category::Places => Some(Intent::ShowPlaceList),
        Category::Sources => Some(Intent::ShowSourceList),
        Category::Citations => Some(Intent::ShowCitationList),
        Category::Repositories => Some(Intent::ShowRepositoryList),
        Category::Media => Some(Intent::ShowMediaList),
        Category::Notes => Some(Intent::ShowNoteList),
        Category::DnaTests => Some(Intent::ShowDnaTestList),
        Category::DnaMatches => Some(Intent::ShowDnaMatchList),
    }
}

#[cfg(test)]
mod tests {
    use super::{ActiveMove, PICKER_MAX_ROWS, PickerSelection, PickerState, list_intent, next_active, picker_rows};
    use crate::list::RowVm;
    use crate::navigation::{Category, Intent};

    fn row(id: &str, title: &str) -> RowVm {
        RowVm {
            id: id.to_owned(),
            title: title.to_owned(),
            ..RowVm::default()
        }
    }

    fn options() -> Vec<RowVm> {
        vec![
            row("I0001", "Ada Lovelace"),
            row("I0002", "Charles Babbage"),
            row("I0003", "Grace Hopper"),
        ]
    }

    #[test]
    fn empty_query_returns_all_options() {
        let rows = picker_rows(&options(), "", &[]);
        assert_eq!(rows.len(), 3);
    }

    #[test]
    fn a_query_filters_case_insensitively_via_row_matches() {
        let rows = picker_rows(&options(), "GRACE", &[]);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, "I0003");
    }

    #[test]
    fn excluded_ids_are_dropped() {
        let rows = picker_rows(&options(), "", &["I0002".to_owned()]);
        let ids: Vec<&str> = rows.iter().map(|r| r.id.as_str()).collect();
        assert_eq!(ids, ["I0001", "I0003"]);
    }

    #[test]
    fn results_are_capped_at_the_max() {
        let many: Vec<RowVm> = (0..20).map(|n| row(&format!("I{n:04}"), "Same Name")).collect();
        let rows = picker_rows(&many, "", &[]);
        assert_eq!(rows.len(), PICKER_MAX_ROWS);
    }

    #[test]
    fn selection_display_shows_title_and_id_or_bare_id() {
        let titled = PickerSelection {
            human_id: "I0042".to_owned(),
            title: "Ada Lovelace".to_owned(),
        };
        assert_eq!(titled.display(), "Ada Lovelace (I0042)");
        let untitled = PickerSelection {
            human_id: "I0042".to_owned(),
            title: "I0042".to_owned(),
        };
        assert_eq!(untitled.display(), "I0042");
    }

    #[test]
    fn pick_records_the_selection_and_closes_the_list() {
        let mut state = PickerState {
            query: "grace".to_owned(),
            open: true,
            selection: None,
        };
        state.pick(&row("I0003", "Grace Hopper"));
        let selection = state.selection.expect("a selection after pick");
        assert_eq!(selection.human_id, "I0003");
        assert_eq!(selection.title, "Grace Hopper");
        assert!(state.query.is_empty(), "the query is cleared on pick");
        assert!(!state.open, "the list closes on pick");
    }

    #[test]
    fn clear_drops_the_selection_and_query() {
        let mut state = PickerState {
            query: "ada".to_owned(),
            open: true,
            selection: Some(PickerSelection {
                human_id: "I0001".to_owned(),
                title: "Ada Lovelace".to_owned(),
            }),
        };
        state.clear();
        assert!(state.selection.is_none());
        assert!(state.query.is_empty());
        assert!(!state.open);
    }

    #[test]
    fn next_active_moves_down_from_mid() {
        assert_eq!(next_active(2, ActiveMove::Down, 6), 3);
    }

    #[test]
    fn next_active_down_clamps_at_the_last_row() {
        assert_eq!(next_active(5, ActiveMove::Down, 6), 5);
    }

    #[test]
    fn next_active_up_clamps_at_the_first_row() {
        assert_eq!(next_active(0, ActiveMove::Up, 6), 0);
    }

    #[test]
    fn next_active_first_jumps_to_the_first_row() {
        assert_eq!(next_active(4, ActiveMove::First, 6), 0);
    }

    #[test]
    fn next_active_last_jumps_to_the_last_row() {
        assert_eq!(next_active(1, ActiveMove::Last, 6), 5);
    }

    #[test]
    fn next_active_is_zero_when_there_are_no_rows() {
        assert_eq!(next_active(3, ActiveMove::Down, 0), 0);
    }

    #[test]
    fn list_intent_covers_every_pickable_category_and_excludes_tags_and_dashboard() {
        for category in Category::all() {
            let pickable = category != Category::Dashboard && category != Category::Tags;
            assert_eq!(
                list_intent(category).is_some(),
                pickable,
                "{category:?} pickability maps to a list intent"
            );
        }
        assert_eq!(list_intent(Category::People), Some(Intent::ShowList));
        assert_eq!(list_intent(Category::Sources), Some(Intent::ShowSourceList));
        assert_eq!(list_intent(Category::DnaMatches), Some(Intent::ShowDnaMatchList));
    }
}
