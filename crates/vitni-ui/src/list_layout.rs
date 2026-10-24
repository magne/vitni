//! An Explorer list's layout (ADR 0052 §7): the columns a row shows after its title and the sort, per
//! kind. A row keeps two lines — the title column, then the chosen columns' cells in order, joined by
//! ` · `, an empty cell skipped. The layout is presentation state, stored per kind in `workspace.toml`
//! (`[ui.lists.<kind>]`) as a [`ListPreferences`], read back leniently: a column the catalogue no
//! longer has is dropped, never refused.

use std::collections::HashMap;
use std::hash::BuildHasher;

use vitni_app::{ColumnType, ListCell, ListPreferences, ListQuery, ListSort, ListedKind, PlaceTypeKey, SortDirection};

use crate::i18n::Localizer;
use crate::view_model::choice_options;

/// The columns a row of `kind` shows after its title when the user has chosen none.
#[must_use]
pub const fn default_columns(kind: ListedKind) -> &'static [&'static str] {
    match kind {
        ListedKind::Person => &["birth", "death"],
        ListedKind::Family => &["marriage", "children"],
        ListedKind::Event => &["date", "place"],
        ListedKind::Place => &["type", "enclosed-by"],
        ListedKind::Source => &["author"],
        ListedKind::Citation => &["page"],
        ListedKind::Repository | ListedKind::Note => &["type"],
        ListedKind::Media => &["mime", "date"],
        ListedKind::ResearchNote => &[],
        ListedKind::DnaTest => &["provider", "test-type"],
        ListedKind::DnaMatch => &["shared-cm", "status"],
        ListedKind::Tag => &["priority"],
    }
}

/// One list's chosen columns and sort, every id checked against the kind's catalogue.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListLayout {
    /// The listed kind.
    pub kind: ListedKind,
    /// The columns a row shows after its title, in order; never the title column.
    pub columns: Vec<String>,
    /// The sort; `None` orders by `human_id`.
    pub sort: Option<ListSort>,
}

impl ListLayout {
    /// The layout `stored` describes for `kind`, or the kind's default where it says nothing. A column
    /// the catalogue lacks, the title column and a repeat are dropped; a sort by a column that is
    /// missing or cannot be sorted falls back to the `human_id` order.
    #[must_use]
    pub fn resolve(kind: ListedKind, stored: Option<&ListPreferences>) -> Self {
        let chosen: Vec<String> = match stored.and_then(|stored| stored.columns.as_ref()) {
            Some(columns) => columns.clone(),
            None => default_columns(kind).iter().map(|id| (*id).to_owned()).collect(),
        };
        let title = kind.title_column().id;
        let mut columns: Vec<String> = Vec::with_capacity(chosen.len());
        for id in chosen {
            if id != title && kind.column(&id).is_some() && !columns.contains(&id) {
                columns.push(id);
            }
        }
        let sort = stored
            .and_then(|stored| stored.sort.clone())
            .filter(|sort| is_sortable(kind, &sort.column));
        Self { kind, columns, sort }
    }

    /// The layout as it is stored: the columns always written out, so hiding every column survives.
    #[must_use]
    pub fn preferences(&self) -> ListPreferences {
        ListPreferences {
            columns: Some(self.columns.clone()),
            sort: self.sort.clone(),
        }
    }

    /// Shows `column` after the shown ones, or hides it if shown. The title column and an id the
    /// catalogue lacks are left alone.
    pub fn toggle(&mut self, column: &str) {
        if let Some(position) = self.columns.iter().position(|shown| shown == column) {
            self.columns.remove(position);
        } else if column != self.kind.title_column().id && self.kind.column(column).is_some() {
            self.columns.push(column.to_owned());
        }
    }

    /// Moves the shown `column` one place earlier (`earlier`) or later; at either end, or not shown,
    /// it stays.
    pub fn shift(&mut self, column: &str, earlier: bool) {
        let Some(position) = self.columns.iter().position(|shown| shown == column) else {
            return;
        };
        let target = if earlier {
            position.checked_sub(1)
        } else {
            Some(position + 1).filter(|next| *next < self.columns.len())
        };
        if let Some(target) = target {
            self.columns.swap(position, target);
        }
    }

    /// Sorts by `column` (`None`: the `human_id` order), keeping the direction already chosen. A column
    /// that cannot be sorted is ignored.
    pub fn sort_by(&mut self, column: Option<&str>) {
        let Some(column) = column else {
            self.sort = None;
            return;
        };
        if !is_sortable(self.kind, column) {
            return;
        }
        let direction = self
            .sort
            .as_ref()
            .map_or(SortDirection::Ascending, |sort| sort.direction);
        self.sort = Some(ListSort {
            column: column.to_owned(),
            direction,
        });
    }

    /// Sets the direction of a column sort; the `human_id` order has only one.
    pub fn sort_direction(&mut self, direction: SortDirection) {
        if let Some(sort) = &mut self.sort {
            sort.direction = direction;
        }
    }
}

/// The layout a saved search brings to the list: its columns and sort where it stores them, the
/// kind's defaults (the default columns, the `human_id` order) where it does not, so applying one
/// restores what was saved whatever the list showed before. Every id is checked against the
/// catalogue, so a retired column drops. A sort the catalogue cannot take is dropped when the saved
/// search is resolved and falls back to the `human_id` order.
#[must_use]
pub fn saved_layout(query: &ListQuery) -> ListLayout {
    let preferences = ListPreferences {
        columns: query.columns.clone(),
        sort: query.sort.clone(),
    };
    ListLayout::resolve(query.kind, Some(&preferences))
}

/// The preferences to write for `current`, or `None` when nothing needs writing: `current` is what is
/// stored already, or is the layout a saved search applied (which belongs to the saved search, not to
/// the list's own preferences).
#[must_use]
pub fn layout_to_store(
    current: &ListLayout,
    stored: &ListLayout,
    applied: Option<&ListLayout>,
) -> Option<ListPreferences> {
    if current == stored || applied == Some(current) {
        return None;
    }
    Some(current.preferences())
}

/// Whether `kind` can be sorted by `column`: every catalogue column but a tag set.
fn is_sortable(kind: ListedKind, column: &str) -> bool {
    kind.column(column).is_some_and(|found| found.ty != ColumnType::Tags)
}

/// One column in the *Columns* menu.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ColumnToggle {
    /// The column id.
    pub id: &'static str,
    /// The localized label.
    pub label: String,
    /// Whether rows show it.
    pub shown: bool,
}

/// The columns the *Columns* menu offers for `layout`: the shown ones in their order, then the hidden
/// ones in catalogue order. The title column is not offered — every row shows it.
#[must_use]
pub fn column_toggles(loc: &Localizer, layout: &ListLayout) -> Vec<ColumnToggle> {
    let kind = layout.kind;
    let title = kind.title_column().id;
    let mut toggles = Vec::with_capacity(kind.columns().len());
    for shown in &layout.columns {
        if let Some(column) = kind.column(shown) {
            toggles.push(ColumnToggle {
                id: column.id,
                label: loc.list_column_label(column.id),
                shown: true,
            });
        }
    }
    for column in kind.columns() {
        if column.id != title && !layout.columns.iter().any(|shown| shown == column.id) {
            toggles.push(ColumnToggle {
                id: column.id,
                label: loc.list_column_label(column.id),
                shown: false,
            });
        }
    }
    toggles
}

/// The columns `kind` can be sorted by, labelled, in catalogue order.
#[must_use]
pub fn sort_columns(loc: &Localizer, kind: ListedKind) -> Vec<(&'static str, String)> {
    let mut columns = Vec::new();
    for column in kind.columns() {
        if column.ty != ColumnType::Tags {
            columns.push((column.id, loc.list_column_label(column.id)));
        }
    }
    columns
}

/// The row line `cells` make for `columns` of `kind`: each non-empty cell's text, joined by ` · `;
/// `None` when every cell is empty. `tags` names a tag by its id; a tag it cannot name reads as
/// unnamed, never as its id.
#[must_use]
pub fn cells_line<S: BuildHasher>(
    loc: &Localizer,
    kind: ListedKind,
    columns: &[String],
    cells: &[Option<ListCell>],
    tags: &HashMap<String, String, S>,
) -> Option<String> {
    let mut parts = Vec::with_capacity(cells.len());
    for (column, cell) in columns.iter().zip(cells) {
        if let Some(text) = cell.as_ref().and_then(|cell| cell_text(loc, kind, column, cell, tags)) {
            parts.push(text);
        }
    }
    (!parts.is_empty()).then(|| parts.join(" · "))
}

/// The label of a choice column's `key`, the key itself when no option carries it.
fn choice_label(loc: &Localizer, kind: ListedKind, column: &str, key: &str) -> String {
    choice_options(loc, kind, column)
        .into_iter()
        .find(|option| option.key == key)
        .map_or_else(|| key.to_owned(), |option| option.label)
}

/// One cell as a row shows it; `None` when it has nothing to show. A number reads with its column's
/// label (`Children 3`), which a bare figure would not tell.
fn cell_text<S: BuildHasher>(
    loc: &Localizer,
    kind: ListedKind,
    column: &str,
    cell: &ListCell,
    tags: &HashMap<String, String, S>,
) -> Option<String> {
    let text = match cell {
        ListCell::Text(text) if kind == ListedKind::Place && column == "type" => place_type_cell(loc, text.trim()),
        ListCell::Text(text) => text.trim().to_owned(),
        ListCell::Date(date) => loc.date(date),
        ListCell::Number(number) => loc.list_number_cell(&loc.list_column_label(column), &number.to_string()),
        ListCell::Choice(key) => choice_label(loc, kind, column, key),
        ListCell::Tags(ids) => {
            let mut names = Vec::with_capacity(ids.len());
            for id in ids {
                names.push(loc.display_name(tags.get(&id.to_string()).map(String::as_str)));
            }
            names.join(", ")
        }
        ListCell::Record { title, .. } => title.clone().unwrap_or_default(),
    };
    (!text.is_empty()).then_some(text)
}

/// A place's `type` cell: its key's label for a `std` or `custom:` key, any other key as itself, so a key
/// never reads as a bare `std:farm` (ADR 0031 §4).
fn place_type_cell(loc: &Localizer, key: &str) -> String {
    PlaceTypeKey::new(key)
        .ok()
        .and_then(|key| loc.place_key_label(&key))
        .unwrap_or_else(|| key.to_owned())
}

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    use uuid::Uuid;
    use vitni_app::{
        DateParts, ListCell, ListPreferences, ListQuery, ListSort, ListedKind, SortDirection, TagId, gregorian_date,
    };

    use super::{
        ColumnToggle, ListLayout, cells_line, column_toggles, default_columns, layout_to_store, saved_layout,
        sort_columns,
    };
    use crate::i18n::Localizer;

    fn loc() -> Localizer {
        Localizer::for_test("en")
    }

    fn ids(columns: &[&str]) -> Vec<String> {
        columns.iter().map(|id| (*id).to_owned()).collect()
    }

    fn sort(column: &str, direction: SortDirection) -> ListSort {
        ListSort {
            column: column.to_owned(),
            direction,
        }
    }

    #[test]
    fn every_default_column_is_in_its_kinds_catalogue_and_not_the_title() {
        for kind in ListedKind::ALL {
            for id in default_columns(*kind) {
                assert!(kind.column(id).is_some(), "{kind:?} has no `{id}`");
                assert_ne!(*id, kind.title_column().id, "{kind:?} defaults to its title column");
            }
        }
    }

    #[test]
    fn nothing_stored_is_the_kinds_default_in_id_order() {
        let layout = ListLayout::resolve(ListedKind::Person, None);
        assert_eq!(layout.columns, ids(&["birth", "death"]));
        assert_eq!(layout.sort, None);
    }

    #[test]
    fn a_stored_layout_drops_what_the_catalogue_cannot_take() {
        let stored = ListPreferences {
            columns: Some(ids(&["death", "name", "eye-colour", "death", "sex"])),
            sort: Some(sort("tags", SortDirection::Descending)),
        };
        let layout = ListLayout::resolve(ListedKind::Person, Some(&stored));
        assert_eq!(
            layout.columns,
            ids(&["death", "sex"]),
            "unknown, title and repeat dropped"
        );
        assert_eq!(layout.sort, None, "a tag set cannot be sorted");
        let gone = ListPreferences {
            columns: None,
            sort: Some(sort("eye-colour", SortDirection::Ascending)),
        };
        assert_eq!(ListLayout::resolve(ListedKind::Person, Some(&gone)).sort, None);
    }

    #[test]
    fn hiding_every_column_survives_a_round_trip() {
        let mut layout = ListLayout::resolve(ListedKind::Event, None);
        layout.toggle("date");
        layout.toggle("place");
        assert_eq!(layout.columns, Vec::<String>::new());
        let stored = layout.preferences();
        assert_eq!(ListLayout::resolve(ListedKind::Event, Some(&stored)), layout);
    }

    #[test]
    fn toggling_appends_or_removes_and_never_takes_the_title() {
        let mut layout = ListLayout::resolve(ListedKind::Person, None);
        layout.toggle("sex");
        layout.toggle("name");
        layout.toggle("nonsense");
        assert_eq!(layout.columns, ids(&["birth", "death", "sex"]));
        layout.toggle("birth");
        assert_eq!(layout.columns, ids(&["death", "sex"]));
    }

    #[test]
    fn shifting_moves_one_place_and_stops_at_the_ends() {
        let mut layout = ListLayout::resolve(ListedKind::Person, None);
        layout.toggle("sex");
        layout.shift("sex", true);
        assert_eq!(layout.columns, ids(&["birth", "sex", "death"]));
        layout.shift("birth", true);
        layout.shift("death", false);
        layout.shift("given", true);
        assert_eq!(layout.columns, ids(&["birth", "sex", "death"]));
        layout.shift("birth", false);
        assert_eq!(layout.columns, ids(&["sex", "birth", "death"]));
    }

    #[test]
    fn sorting_keeps_the_direction_and_the_id_order_has_none() {
        let mut layout = ListLayout::resolve(ListedKind::Person, None);
        layout.sort_direction(SortDirection::Descending);
        assert_eq!(layout.sort, None, "the id order has one direction");
        layout.sort_by(Some("birth"));
        layout.sort_direction(SortDirection::Descending);
        layout.sort_by(Some("surname"));
        assert_eq!(layout.sort, Some(sort("surname", SortDirection::Descending)));
        layout.sort_by(Some("tags"));
        assert_eq!(
            layout.sort,
            Some(sort("surname", SortDirection::Descending)),
            "tags ignored"
        );
        layout.sort_by(None);
        assert_eq!(layout.sort, None);
    }

    #[test]
    fn the_menu_lists_shown_columns_in_order_then_the_hidden_ones() {
        let mut layout = ListLayout::resolve(ListedKind::Person, None);
        layout.shift("death", true);
        let toggles = column_toggles(&loc(), &layout);
        let shown: Vec<(&str, bool)> = toggles.iter().map(|toggle| (toggle.id, toggle.shown)).collect();
        assert_eq!(
            shown,
            [
                ("death", true),
                ("birth", true),
                ("given", false),
                ("surname", false),
                ("sex", false),
                ("tags", false),
            ]
        );
        assert_eq!(
            toggles[0],
            ColumnToggle {
                id: "death",
                label: "Death".to_owned(),
                shown: true
            }
        );
    }

    #[test]
    fn every_column_but_a_tag_set_sorts() {
        let columns: Vec<&str> = sort_columns(&loc(), ListedKind::Person)
            .into_iter()
            .map(|(id, _)| id)
            .collect();
        assert_eq!(columns, ["name", "given", "surname", "sex", "birth", "death"]);
    }

    #[test]
    fn a_line_joins_the_non_empty_cells_each_as_its_type_reads() {
        let tag = TagId::from_uuid(Uuid::from_u128(7));
        let tags = HashMap::from([(tag.to_string(), "To verify".to_owned())]);
        let columns = ids(&["birth", "death", "sex", "tags"]);
        let cells = vec![
            Some(ListCell::Date(gregorian_date(DateParts {
                year: 1850,
                month: None,
                day: None,
            }))),
            None,
            Some(ListCell::Choice("female".to_owned())),
            Some(ListCell::Tags(vec![tag])),
        ];
        assert_eq!(
            cells_line(&loc(), ListedKind::Person, &columns, &cells, &tags).as_deref(),
            Some("1850 · female · To verify")
        );
        let unnamed = TagId::from_uuid(Uuid::from_u128(8));
        let line = cells_line(
            &loc(),
            ListedKind::Person,
            &ids(&["tags"]),
            &[Some(ListCell::Tags(vec![unnamed]))],
            &tags,
        )
        .expect("a tag cell");
        assert!(
            !line.contains(&unnamed.to_string()),
            "a tag's id is never shown: {line}"
        );
        assert_eq!(line, loc().display_name(None));
        let family = cells_line(
            &loc(),
            ListedKind::Family,
            &ids(&["children"]),
            &[Some(ListCell::Number(3.0))],
            &tags,
        );
        assert_eq!(family.as_deref(), Some("Children 3"));
        let place = cells_line(
            &loc(),
            ListedKind::Event,
            &ids(&["place", "description"]),
            &[
                Some(ListCell::Record {
                    id: "x".to_owned(),
                    title: Some("Bergen".to_owned()),
                }),
                Some(ListCell::Text("  ".to_owned())),
            ],
            &tags,
        );
        assert_eq!(place.as_deref(), Some("Bergen"), "a blank text is an empty cell");
        assert_eq!(
            cells_line(&loc(), ListedKind::Person, &columns, &[None, None], &tags),
            None
        );
    }

    #[test]
    fn the_place_type_cell_shows_the_keys_label_and_never_a_bare_std_key() {
        let tags = HashMap::new();
        let line = |loc: &Localizer, key: &str| {
            cells_line(
                loc,
                ListedKind::Place,
                &ids(&["type"]),
                &[Some(ListCell::Text(key.to_owned()))],
                &tags,
            )
        };
        assert_eq!(line(&loc(), "std:farm").as_deref(), Some("Farm"));
        assert_eq!(
            line(&Localizer::for_test("no"), "std:farm").as_deref(),
            Some("G\u{e5}rd")
        );
        assert_eq!(line(&loc(), "custom:Sokn").as_deref(), Some("Sokn"));
        assert_eq!(
            line(&loc(), "no:prestegjeld").as_deref(),
            Some("no:prestegjeld"),
            "another namespace shows its key, not kind and key"
        );
    }

    fn person_layout(columns: &[&str], sort: Option<ListSort>) -> ListLayout {
        ListLayout {
            kind: ListedKind::Person,
            columns: ids(columns),
            sort,
        }
    }

    #[test]
    fn a_layout_equal_to_the_stored_one_is_not_written() {
        let stored = person_layout(&["birth", "death"], None);
        let applied = person_layout(&["sex"], None);
        assert_eq!(layout_to_store(&stored.clone(), &stored, Some(&applied)), None);
        assert_eq!(layout_to_store(&stored.clone(), &stored, None), None);
    }

    #[test]
    fn a_layout_a_saved_search_applied_is_not_written() {
        let stored = person_layout(&["birth", "death"], None);
        let applied = person_layout(&["sex"], Some(sort("birth", SortDirection::Descending)));
        assert_eq!(layout_to_store(&applied.clone(), &stored, Some(&applied)), None);
    }

    #[test]
    fn a_layout_the_user_changed_after_applying_is_written() {
        let stored = person_layout(&["birth", "death"], None);
        let applied = person_layout(&["sex"], None);
        let current = person_layout(&["sex", "death"], None);
        assert_eq!(
            layout_to_store(&current, &stored, Some(&applied)),
            Some(current.preferences())
        );
        assert_eq!(layout_to_store(&current, &stored, None), Some(current.preferences()));
    }

    #[test]
    fn a_saved_query_without_columns_or_sort_brings_the_kind_defaults() {
        let query = ListQuery::new(ListedKind::Person);
        assert_eq!(saved_layout(&query), ListLayout::resolve(ListedKind::Person, None));
    }

    #[test]
    fn a_saved_query_columns_and_sort_come_through_minus_retired_ids() {
        let query = ListQuery {
            columns: Some(ids(&["death", "eye-colour", "name"])),
            sort: Some(sort("birth", SortDirection::Descending)),
            ..ListQuery::new(ListedKind::Person)
        };
        let layout = saved_layout(&query);
        assert_eq!(layout.columns, ids(&["death"]), "unknown and title columns dropped");
        assert_eq!(layout.sort, Some(sort("birth", SortDirection::Descending)));
    }

    #[test]
    fn a_search_saved_with_every_column_hidden_comes_back_with_none() {
        let hidden = person_layout(&[], None);
        let query = ListQuery {
            columns: Some(hidden.columns.clone()),
            ..ListQuery::new(ListedKind::Person)
        };
        assert_eq!(saved_layout(&query).columns, hidden.columns);
    }

    #[test]
    fn a_saved_sort_the_catalogue_cannot_take_falls_back_to_no_sort() {
        let query = ListQuery {
            columns: Some(ids(&["death"])),
            sort: Some(sort("tags", SortDirection::Ascending)),
            ..ListQuery::new(ListedKind::Person)
        };
        assert_eq!(saved_layout(&query).sort, None);
    }
}
