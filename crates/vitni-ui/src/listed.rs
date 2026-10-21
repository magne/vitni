//! A list read from the store a page at a time (ADR 0052 §4): the rows of a [`ListQuery`] the store has
//! evaluated, held by page under the total it matched. The Explorer's list box is as tall as the total
//! (#590), so a window anywhere in it names the pages it needs, and `[`/`]` steps over the whole result,
//! fetched or not.

use std::collections::{BTreeMap, HashMap};

use vitni_app::{
    AppError, ColumnType, FilterValue, ListFilter, ListPage, ListQuery, ListedKind, ListedPage, ReferenceLabel,
    SavedSearchResolution, Workspace, list_citations_page, list_dna_matches_page, list_dna_tests_page,
    list_events_page, list_families_page, list_media_page, list_notes_page, list_persons_page, list_places_page,
    list_repositories_page, list_research_notes_page, list_sources_page, list_tags, list_tags_query, reference_id,
    reference_label,
};

use crate::i18n::Localizer;
use crate::list::RowVm;
use crate::list_layout::cells_line;
use crate::list_window::WINDOW_ROWS;
use crate::navigation::Category;
use crate::view_model::{
    ActiveFilter, FilterDraft, citation_row, dna_match_row, dna_test_row, event_row, family_row, media_row, note_row,
    person_row, place_row, repository_row, research_note_row, source_row, tag_row,
};

/// How many rows one fetched page holds: a window's worth, so a window spans at most two pages.
pub const PAGE_ROWS: usize = WINDOW_ROWS;

/// Rows `offset..offset + rows.len()` of a query's result, and the total it matched.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct RowsPage {
    /// The index of the first row.
    pub offset: usize,
    /// How many records the query matched.
    pub total: usize,
    /// The rows, in the query's order.
    pub rows: Vec<RowVm>,
}

/// The fetched pages of one query's result, under its total.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ListedRows {
    total: usize,
    pages: BTreeMap<usize, Vec<RowVm>>,
}

impl ListedRows {
    /// A result held whole, already in memory (the Help topics).
    #[must_use]
    pub fn whole(rows: Vec<RowVm>) -> Self {
        let mut listed = Self::default();
        listed.insert(&RowsPage {
            offset: 0,
            total: rows.len(),
            rows,
        });
        listed
    }

    /// Adds a fetched page, taking its total as the result's. `page.offset` is a multiple of
    /// [`PAGE_ROWS`]; rows past one page fill the pages after it.
    pub fn insert(&mut self, page: &RowsPage) {
        self.total = page.total;
        let first = page.offset / PAGE_ROWS;
        for (step, chunk) in page.rows.chunks(PAGE_ROWS).enumerate() {
            self.pages.insert(first + step, chunk.to_vec());
        }
    }

    /// How many records the query matched.
    #[must_use]
    pub const fn total(&self) -> usize {
        self.total
    }

    /// Whether the query matched nothing.
    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.total == 0
    }

    /// Row `index`, if its page is fetched.
    #[must_use]
    pub fn row(&self, index: usize) -> Option<&RowVm> {
        self.pages.get(&(index / PAGE_ROWS))?.get(index % PAGE_ROWS)
    }

    /// The index of the fetched row with `id`.
    #[must_use]
    pub fn index_of(&self, id: &str) -> Option<usize> {
        for (page, rows) in &self.pages {
            if let Some(position) = rows.iter().position(|row| row.id == id) {
                return Some(page * PAGE_ROWS + position);
            }
        }
        None
    }

    /// The pages not fetched yet that hold any of the rows `indices`, ascending, each once.
    #[must_use]
    pub fn missing_pages(&self, indices: &[usize]) -> Vec<usize> {
        let mut missing = Vec::new();
        for index in indices {
            let page = index / PAGE_ROWS;
            if *index < self.total && !self.pages.contains_key(&page) && !missing.contains(&page) {
                missing.push(page);
            }
        }
        missing.sort_unstable();
        missing
    }

    /// The index `delta` rows from the `selected` one — the `[`/`]` step over the whole result, its
    /// page fetched or not. Clamps at both ends; with no selection, or one not among the fetched rows, a
    /// forward step lands on the first row and a backward one on the last. `None` for an empty result.
    #[must_use]
    pub fn step(&self, selected: Option<&str>, delta: isize) -> Option<usize> {
        let last = self.total.checked_sub(1)?;
        let Some(current) = selected.and_then(|id| self.index_of(id)) else {
            return Some(if delta >= 0 { 0 } else { last });
        };
        let target = if delta >= 0 {
            current.saturating_add(delta.unsigned_abs())
        } else {
            current.saturating_sub(delta.unsigned_abs())
        };
        Some(target.min(last))
    }
}

/// The store window that reads `page`.
#[must_use]
pub fn page_window(page: usize) -> ListPage {
    ListPage {
        offset: u32::try_from(page * PAGE_ROWS).unwrap_or(u32::MAX),
        limit: u32::try_from(PAGE_ROWS).ok(),
    }
}

/// The kind an entity category lists; `None` for the Dashboard, which lists nothing.
#[must_use]
pub const fn listed_kind(category: Category) -> Option<ListedKind> {
    Some(match category {
        Category::Dashboard => return None,
        Category::People => ListedKind::Person,
        Category::Families => ListedKind::Family,
        Category::Events => ListedKind::Event,
        Category::Places => ListedKind::Place,
        Category::Sources => ListedKind::Source,
        Category::Citations => ListedKind::Citation,
        Category::Repositories => ListedKind::Repository,
        Category::Media => ListedKind::Media,
        Category::Notes => ListedKind::Note,
        Category::ResearchNotes => ListedKind::ResearchNote,
        Category::Tags => ListedKind::Tag,
        Category::DnaTests => ListedKind::DnaTest,
        Category::DnaMatches => ListedKind::DnaMatch,
    })
}

/// The entity category that lists `kind`.
#[must_use]
pub const fn listed_category(kind: ListedKind) -> Category {
    match kind {
        ListedKind::Person => Category::People,
        ListedKind::Family => Category::Families,
        ListedKind::Event => Category::Events,
        ListedKind::Place => Category::Places,
        ListedKind::Source => Category::Sources,
        ListedKind::Citation => Category::Citations,
        ListedKind::Repository => Category::Repositories,
        ListedKind::Media => Category::Media,
        ListedKind::Note => Category::Notes,
        ListedKind::ResearchNote => Category::ResearchNotes,
        ListedKind::Tag => Category::Tags,
        ListedKind::DnaTest => Category::DnaTests,
        ListedKind::DnaMatch => Category::DnaMatches,
    }
}

/// Page `page` of the records `query` lists, as rows. The tag list is read whole (ADR 0052 §4), so for
/// a tag query every row comes back from offset 0, whatever `page` asked for.
///
/// # Errors
///
/// [`AppError::ListQuery`] for a query the catalogue cannot evaluate, or a store/read-model error.
pub async fn list_page(
    workspace: &Workspace,
    loc: &Localizer,
    query: &ListQuery,
    page: usize,
) -> Result<RowsPage, AppError> {
    let window = page_window(page);
    let offset = page * PAGE_ROWS;
    let line = Line {
        loc,
        query,
        tags: tag_names(workspace, query).await?,
    };
    let rows = match query.kind {
        ListedKind::Person => paged(
            &list_persons_page(workspace, query, window).await?,
            offset,
            &line,
            |s| person_row(s, loc),
        ),
        ListedKind::Family => paged(
            &list_families_page(workspace, query, window).await?,
            offset,
            &line,
            |s| family_row(s, loc),
        ),
        ListedKind::Event => paged(&list_events_page(workspace, query, window).await?, offset, &line, |s| {
            event_row(s, loc)
        }),
        ListedKind::Place => paged(&list_places_page(workspace, query, window).await?, offset, &line, |s| {
            place_row(s, loc)
        }),
        ListedKind::Source => paged(
            &list_sources_page(workspace, query, window).await?,
            offset,
            &line,
            |s| source_row(s, loc),
        ),
        ListedKind::Citation => paged(
            &list_citations_page(workspace, query, window).await?,
            offset,
            &line,
            |s| citation_row(s, loc),
        ),
        ListedKind::Repository => paged(
            &list_repositories_page(workspace, query, window).await?,
            offset,
            &line,
            |s| repository_row(s, loc),
        ),
        ListedKind::Media => paged(&list_media_page(workspace, query, window).await?, offset, &line, |s| {
            media_row(s, loc)
        }),
        ListedKind::Note => paged(&list_notes_page(workspace, query, window).await?, offset, &line, |s| {
            note_row(s, loc)
        }),
        ListedKind::ResearchNote => paged(
            &list_research_notes_page(workspace, query, window).await?,
            offset,
            &line,
            |s| research_note_row(s, loc),
        ),
        ListedKind::DnaTest => paged(
            &list_dna_tests_page(workspace, query, window).await?,
            offset,
            &line,
            |s| dna_test_row(s, loc),
        ),
        ListedKind::DnaMatch => paged(
            &list_dna_matches_page(workspace, query, window).await?,
            offset,
            &line,
            |s| dna_match_row(s, loc),
        ),
        ListedKind::Tag => paged(&list_tags_query(workspace, query).await?, 0, &line, |tag| {
            tag_row(tag, loc)
        }),
    };
    Ok(rows)
}

/// `active` with the record it names held by its aggregate id, as the query does (ADR 0052 §3): the
/// editor picks a record by its `human_id`, which a renumbering would break. Any other clause comes back
/// unchanged.
///
/// # Errors
///
/// [`AppError::ListQuery`] when the picked record no longer exists, or a store/read-model error.
pub async fn resolve_filter(
    workspace: &Workspace,
    kind: ListedKind,
    mut active: ActiveFilter,
) -> Result<ActiveFilter, AppError> {
    let is_record = kind
        .column(&active.filter.column)
        .is_some_and(|column| column.ty == ColumnType::Record);
    if let (true, FilterValue::Text(human_id)) = (is_record, &active.filter.value) {
        let id = reference_id(workspace, kind, &active.filter.column, human_id).await?;
        active.filter.value = FilterValue::Text(id);
    }
    Ok(active)
}

/// A saved clause the catalogue cannot take, as its chip shows it (ADR 0052 §10). It is never an
/// [`ActiveFilter`]: a [`ListFilter`] holds only an operator the catalogue knows.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleClauseVm {
    /// The column the clause names, as stored (it may no longer exist); empty when the clause names none.
    pub column: String,
    /// The localized chip text: the clause's column and operator as stored, and why it cannot run.
    pub label: String,
}

/// A saved search's clauses rebuilt for the filter bar.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedFilters {
    /// The clauses that can run, each with the draft that reopens it in the editor.
    pub applied: Vec<ActiveFilter>,
    /// The clauses that cannot. While any exists the search must not run.
    pub stale: Vec<StaleClauseVm>,
}

/// The filter bar's chips for a resolved saved search (ADR 0052 §3, §10): a tag or record held by id
/// shows by its name, a deleted one as such, never by its id. The stored id stays in the clause, so a
/// deleted tag or record still matches nothing.
///
/// # Errors
///
/// A store/read-model error.
pub async fn saved_filters(
    workspace: &Workspace,
    loc: &Localizer,
    resolution: &SavedSearchResolution,
) -> Result<SavedFilters, AppError> {
    let kind = resolution.query.kind;
    let mut applied = Vec::with_capacity(resolution.query.filters.len());
    let mut titles = None;
    for filter in &resolution.query.filters {
        let draft = saved_draft(workspace, loc, kind, filter, &mut titles).await?;
        applied.push(ActiveFilter {
            filter: filter.clone(),
            draft,
        });
    }
    let stale = resolution
        .stale
        .iter()
        .map(|clause| StaleClauseVm {
            column: clause.column.clone().unwrap_or_default(),
            label: match clause.gap() {
                Some(gap) => loc.filter_stale_gapped_clause(&clause.named(" "), gap),
                None => loc.filter_stale_clause(&clause.named(" "), &clause.error),
            },
        })
        .collect();
    Ok(SavedFilters { applied, stale })
}

/// The editor draft that reads back as `filter`. `titles` holds the tag titles once a tag clause has
/// read them, so a saved search lists the tags at most once.
async fn saved_draft(
    workspace: &Workspace,
    loc: &Localizer,
    kind: ListedKind,
    filter: &ListFilter,
    titles: &mut Option<HashMap<String, String>>,
) -> Result<FilterDraft, AppError> {
    let mut draft = FilterDraft::from_filter(kind, filter);
    match &filter.value {
        FilterValue::None
        | FilterValue::Date(_)
        | FilterValue::DateRange { .. }
        | FilterValue::Number(_)
        | FilterValue::NumberRange { .. } => {}
        FilterValue::Text(text) => {
            if draft.value_type(kind) == Some(ColumnType::Record) {
                reference_value(workspace, loc, kind, &mut draft, text).await?;
            }
        }
        FilterValue::Values(values) => {
            if draft.value_type(kind) == Some(ColumnType::Tags) {
                let titles = match titles {
                    Some(titles) => titles,
                    None => titles.insert(tag_titles(workspace).await?),
                };
                for value in values {
                    let text = titles.get(value).cloned().unwrap_or_else(|| loc.filter_deleted_tag());
                    push_value(&mut draft, value.clone(), text);
                }
            } else {
                for value in values {
                    reference_value(workspace, loc, kind, &mut draft, value).await?;
                }
            }
        }
    }
    Ok(draft)
}

/// Adds `id` to the draft's values: a record by what names it (it reopens on its `human_id`), a choice
/// key as it is, and a record nothing has any more as deleted.
async fn reference_value(
    workspace: &Workspace,
    loc: &Localizer,
    kind: ListedKind,
    draft: &mut FilterDraft,
    id: &str,
) -> Result<(), AppError> {
    let column = draft.value_type(kind);
    if column != Some(ColumnType::Record) {
        draft.values.push(id.to_owned());
        return Ok(());
    }
    let label = reference_label(workspace, kind, &draft.column, id).await?;
    push_reference(loc, draft, id, label);
    Ok(())
}

/// Every existing tag's title by id; a tag without a name has an empty title.
async fn tag_titles(workspace: &Workspace) -> Result<HashMap<String, String>, AppError> {
    let mut titles = HashMap::new();
    for tag in list_tags(workspace).await? {
        titles.insert(tag.id, tag.name.unwrap_or_default());
    }
    Ok(titles)
}

/// Pushes a record reference as a value of the draft: by its `human_id` when `label` names it, as deleted
/// (`None`) under its id.
fn push_reference(loc: &Localizer, draft: &mut FilterDraft, id: &str, label: Option<ReferenceLabel>) {
    let (value, text) = match label {
        Some(ReferenceLabel {
            human_id,
            title: Some(title),
        }) => (human_id, title),
        Some(ReferenceLabel { human_id, title: None }) => (human_id.clone(), human_id),
        None => {
            draft.deleted_records.insert(id.to_owned());
            (id.to_owned(), loc.filter_deleted_record())
        }
    };
    push_value(draft, value, text);
}

/// Adds `value` to the draft's values, shown as `text`.
fn push_value(draft: &mut FilterDraft, value: String, text: String) {
    draft.labels.insert(value.clone(), text);
    draft.values.push(value);
}

/// What a row's second line is made of: the query's columns, read as the locale reads them.
struct Line<'a> {
    loc: &'a Localizer,
    query: &'a ListQuery,
    tags: HashMap<String, String>,
}

/// The tag names a tag column's cells are read by — none when the query shows no tag column.
async fn tag_names(workspace: &Workspace, query: &ListQuery) -> Result<HashMap<String, String>, AppError> {
    let shows_tags = query.columns.iter().flatten().any(|id| {
        query
            .kind
            .column(id)
            .is_some_and(|column| column.ty == ColumnType::Tags)
    });
    let mut names = HashMap::new();
    if shows_tags {
        for tag in list_tags(workspace).await? {
            if let Some(name) = tag.name {
                names.insert(tag.id, name);
            }
        }
    }
    Ok(names)
}

/// A use-case's page as rows starting at `offset`, each row's second line its cells (ADR 0052 §7).
fn paged<T>(page: &ListedPage<T>, offset: usize, line: &Line<'_>, row: impl Fn(&T) -> RowVm) -> RowsPage {
    let mut rows = Vec::with_capacity(page.items.len());
    for item in &page.items {
        let mut built = row(item);
        built.subtitle = page.cells.get(&built.id).and_then(|cells| {
            cells_line(
                line.loc,
                line.query.kind,
                line.query.columns.as_deref().unwrap_or_default(),
                cells,
                &line.tags,
            )
        });
        rows.push(built);
    }
    RowsPage {
        offset,
        total: usize::try_from(page.total).unwrap_or(usize::MAX),
        rows,
    }
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;

    fn rows(from: usize, count: usize) -> Vec<RowVm> {
        (from..from + count)
            .map(|index| RowVm {
                id: format!("I{index:04}"),
                title: format!("Row {index}"),
                ..RowVm::default()
            })
            .collect()
    }

    fn page(page: usize, total: usize) -> RowsPage {
        let offset = page * PAGE_ROWS;
        RowsPage {
            offset,
            total,
            rows: rows(offset, PAGE_ROWS.min(total - offset)),
        }
    }

    #[test]
    fn a_page_places_its_rows_at_their_index_in_the_whole_result() {
        let mut listed = ListedRows::default();
        listed.insert(&page(2, 1000));

        assert_eq!(listed.total(), 1000);
        assert_eq!(listed.row(2 * PAGE_ROWS).map(|row| row.id.as_str()), Some("I0128"));
        assert_eq!(listed.row(0), None, "page 0 is not fetched");
        assert_eq!(listed.index_of("I0130"), Some(130));
    }

    #[test]
    fn a_whole_result_fills_every_page() {
        let listed = ListedRows::whole(rows(0, PAGE_ROWS * 2 + 3));

        assert_eq!(listed.total(), PAGE_ROWS * 2 + 3);
        assert_eq!(listed.row(PAGE_ROWS * 2 + 2).map(|row| row.id.as_str()), Some("I0130"));
        assert_eq!(
            listed.missing_pages(&[0, PAGE_ROWS, PAGE_ROWS * 2]),
            Vec::<usize>::new()
        );
    }

    #[test]
    fn a_window_straddling_two_pages_names_each_missing_page_once() {
        let mut listed = ListedRows::default();
        listed.insert(&page(0, 1000));
        let window: Vec<usize> = (PAGE_ROWS - 4..PAGE_ROWS * 3 + 4).collect();

        assert_eq!(listed.missing_pages(&window), [1, 2, 3]);
    }

    #[test]
    fn indices_past_the_total_need_no_page() {
        let mut listed = ListedRows::default();
        listed.insert(&page(0, 10));

        assert_eq!(listed.missing_pages(&[9, 10, PAGE_ROWS + 1]), Vec::<usize>::new());
    }

    #[test]
    fn an_empty_result_has_no_step() {
        let listed = ListedRows::whole(Vec::new());

        assert!(listed.is_empty());
        assert_eq!(listed.step(None, 1), None);
        assert_eq!(listed.step(Some("I0001"), -1), None);
    }

    #[test]
    fn a_step_walks_past_the_fetched_page_and_clamps_at_the_ends() {
        let mut listed = ListedRows::default();
        listed.insert(&page(0, 1000));
        let last_fetched = listed.row(PAGE_ROWS - 1).map(|row| row.id.clone());

        assert_eq!(
            listed.step(last_fetched.as_deref(), 1),
            Some(PAGE_ROWS),
            "into an unfetched page"
        );
        assert_eq!(listed.step(Some("I0000"), -1), Some(0));
        assert_eq!(listed.step(Some("I0000"), 5000), Some(999));
    }

    #[test]
    fn a_step_without_a_fetched_selection_starts_at_an_end() {
        let mut listed = ListedRows::default();
        listed.insert(&page(0, 1000));

        assert_eq!(listed.step(None, 1), Some(0));
        assert_eq!(listed.step(Some("elsewhere"), -1), Some(999));
    }

    #[test]
    fn a_later_page_takes_the_newer_total() {
        let mut listed = ListedRows::default();
        listed.insert(&page(0, 1000));
        listed.insert(&page(1, 900));

        assert_eq!(listed.total(), 900);
    }

    #[test]
    fn listed_kind_covers_every_entity_category() {
        assert_eq!(listed_kind(Category::Dashboard), None);
        assert_eq!(listed_kind(Category::People), Some(ListedKind::Person));
        assert_eq!(listed_kind(Category::Tags), Some(ListedKind::Tag));
        assert_eq!(listed_kind(Category::DnaMatches), Some(ListedKind::DnaMatch));
    }

    #[test]
    fn every_kind_lists_in_the_category_that_lists_it() {
        for kind in ListedKind::ALL {
            assert_eq!(listed_kind(listed_category(*kind)), Some(*kind));
        }
    }

    #[test]
    fn a_page_window_reads_one_page_at_its_offset() {
        assert_eq!(
            page_window(3),
            ListPage {
                offset: u32::try_from(3 * PAGE_ROWS).unwrap_or_default(),
                limit: u32::try_from(PAGE_ROWS).ok(),
            }
        );
    }

    proptest! {
        #[test]
        fn a_step_lands_inside_the_result(total in 1_usize..500, start in 0_usize..500, delta in -600_isize..600) {
            let listed = ListedRows::whole(rows(0, total));
            let selected = format!("I{:04}", start.min(total - 1));
            let target = listed.step(Some(&selected), delta);
            prop_assert!(target.is_some_and(|index| index < total));
        }

        #[test]
        fn every_missing_page_holds_a_requested_unfetched_row(
            fetched in proptest::collection::btree_set(0_usize..8, 0..8),
            indices in proptest::collection::vec(0_usize..600, 0..80),
        ) {
            let total = 500;
            let mut listed = ListedRows::default();
            for page in fetched {
                if page * PAGE_ROWS < total {
                    listed.insert(&self::page(page, total));
                }
            }
            for page in listed.missing_pages(&indices) {
                prop_assert!(listed.row(page * PAGE_ROWS).is_none());
                prop_assert!(indices.iter().any(|index| index / PAGE_ROWS == page && *index < total));
            }
        }
    }
}
