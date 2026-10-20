//! List queries (ADR 0052): the serde [`ListQuery`] a list, a saved search and the CLI share, checked
//! against the listed kind's typed column catalogue and evaluated by the store.
//!
//! The catalogue itself — each kind's column ids and value types, its first column the title — is
//! [`ListedKind::columns`], declared beside the list tables that hold the values (`vitni-db`) and
//! re-exported here as the app's. A query names columns and operators by id and holds tag and record
//! values by id, never by a localized label or display name, so it survives a language switch and a
//! rename (§3).

use std::collections::HashMap;

use serde::{Deserialize, Serialize};
use vitni_core::date::{DateModifier, DatePoint, GenealogicalDate, GenealogicalDateBody};
use vitni_core::dna::{DnaProvider, DnaTestType};
use vitni_core::dna_match::MatchStatus;
use vitni_core::enums::{EventType, NoteType, PlaceType, RepositoryType, Sex};
use vitni_core::provenance::Confidence;
use vitni_db::{
    ColumnType, ListCell, ListPage, ListPredicate, ListResult, ListSpec, ListTest, ListWindow, ListedKind,
    SortDirection, choice_name,
};

use crate::error::AppError;
use crate::workspace::Workspace;

/// A filter operator. Which ones a column takes is fixed by its value type (ADR 0052 §2).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FilterOp {
    /// Text: contains the value.
    Contains,
    /// Text: is the value. Record: is the record.
    Is,
    /// Text or date: has no value.
    IsEmpty,
    /// Date: before the value's period.
    Before,
    /// Date: after the value's period.
    After,
    /// Date or number: within the range, both ends included.
    Between,
    /// Number: less than the value.
    LessThan,
    /// Number: greater than the value.
    GreaterThan,
    /// Choice: one of the values.
    IsAnyOf,
    /// Choice: none of the values (a record with no value passes).
    IsNoneOf,
    /// Tags: carries one of the tags.
    HasAnyOf,
    /// Tags: carries every one of the tags.
    HasAllOf,
    /// Tags: carries none of the tags.
    HasNoneOf,
}

impl FilterOp {
    /// Every operator.
    pub const ALL: &'static [Self] = &[
        Self::Contains,
        Self::Is,
        Self::IsEmpty,
        Self::Before,
        Self::After,
        Self::Between,
        Self::LessThan,
        Self::GreaterThan,
        Self::IsAnyOf,
        Self::IsNoneOf,
        Self::HasAnyOf,
        Self::HasAllOf,
        Self::HasNoneOf,
    ];

    /// The operator's name as a query and the CLI spell it (`is-any-of`).
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Contains => "contains",
            Self::Is => "is",
            Self::IsEmpty => "is-empty",
            Self::Before => "before",
            Self::After => "after",
            Self::Between => "between",
            Self::LessThan => "less-than",
            Self::GreaterThan => "greater-than",
            Self::IsAnyOf => "is-any-of",
            Self::IsNoneOf => "is-none-of",
            Self::HasAnyOf => "has-any-of",
            Self::HasAllOf => "has-all-of",
            Self::HasNoneOf => "has-none-of",
        }
    }

    /// The operator spelled `name`, if any.
    #[must_use]
    pub fn parse(name: &str) -> Option<Self> {
        Self::ALL.iter().copied().find(|op| op.as_str() == name)
    }

    /// The operators a column of type `ty` takes.
    #[must_use]
    pub const fn for_type(ty: ColumnType) -> &'static [Self] {
        match ty {
            ColumnType::Text => &[Self::Contains, Self::Is, Self::IsEmpty],
            ColumnType::Date => &[Self::Before, Self::After, Self::Between, Self::IsEmpty],
            ColumnType::Number => &[Self::LessThan, Self::GreaterThan, Self::Between],
            ColumnType::Choice => &[Self::IsAnyOf, Self::IsNoneOf],
            ColumnType::Tags => &[Self::HasAnyOf, Self::HasAllOf, Self::HasNoneOf],
            ColumnType::Record => &[Self::Is],
        }
    }
}

impl std::fmt::Display for FilterOp {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A filter's value; which shape an operator needs follows from it (`between` on a date takes a
/// [`FilterValue::DateRange`]).
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
#[serde(tag = "type", content = "value", rename_all = "kebab-case")]
pub enum FilterValue {
    /// No value: `is-empty`.
    #[default]
    None,
    /// A string: text `contains`/`is`, or a record's aggregate id for `is`.
    Text(String),
    /// A date, compared by its period.
    Date(GenealogicalDate),
    /// Two dates, the range from the first's period to the second's.
    DateRange {
        /// The range's start.
        from: GenealogicalDate,
        /// The range's end.
        to: GenealogicalDate,
    },
    /// A number.
    Number(f64),
    /// Two numbers, both ends included.
    NumberRange {
        /// The range's low end.
        from: f64,
        /// The range's high end.
        to: f64,
    },
    /// Choice values (`female`) or tag ids.
    Values(Vec<String>),
}

/// One filter clause: a column, an operator, and the value it compares with.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListFilter {
    /// The column id.
    pub column: String,
    /// The operator.
    pub op: FilterOp,
    /// The value.
    #[serde(default)]
    pub value: FilterValue,
}

/// A sort: one column and a direction; ties break by `human_id`, then the record id.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ListSort {
    /// The column id.
    pub column: String,
    /// The direction.
    pub direction: SortDirection,
}

/// A list query (ADR 0052 §3): what to list, how to narrow and order it, and which columns to return.
/// Serde, and changed only by addition, like an event payload (ADR 0004 §4): a saved search stores it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ListQuery {
    /// The kind listed.
    pub kind: ListedKind,
    /// A case-insensitive substring of a text column or the `human_id`; empty matches everything.
    #[serde(default)]
    pub search: String,
    /// Filters, all of which a record must pass.
    #[serde(default)]
    pub filters: Vec<ListFilter>,
    /// The sort; `None` orders by `human_id`.
    #[serde(default)]
    pub sort: Option<ListSort>,
    /// The column ids each row returns, in order.
    #[serde(default)]
    pub columns: Vec<String>,
}

impl ListQuery {
    /// Lists every record of `kind` in `human_id` order, with no columns.
    #[must_use]
    pub const fn new(kind: ListedKind) -> Self {
        Self {
            kind,
            search: String::new(),
            filters: Vec::new(),
            sort: None,
            columns: Vec::new(),
        }
    }
}

/// A query the catalogue cannot evaluate. It does not run: dropping a clause would return more
/// records than it asked for (ADR 0052 §10).
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum ListQueryError {
    /// The kind has no such column.
    #[error("the {} list has no column `{column}`", kind.as_str())]
    UnknownColumn {
        /// The kind queried.
        kind: ListedKind,
        /// The column named.
        column: String,
    },
    /// No operator has the name.
    #[error("the `{column}` filter names no operator `{op}`")]
    UnknownOperator {
        /// The column.
        column: String,
        /// The name given.
        op: String,
    },
    /// The column's type does not take the operator.
    #[error("the `{column}` column takes no `{op}` filter")]
    UnsupportedOperator {
        /// The column.
        column: String,
        /// The operator.
        op: FilterOp,
    },
    /// The value is not the shape the operator needs.
    #[error("the `{column}` `{op}` filter has a value of the wrong kind")]
    WrongValue {
        /// The column.
        column: String,
        /// The operator.
        op: FilterOp,
    },
    /// A choice column has no such value.
    #[error("`{value}` is not a value of the `{column}` column")]
    UnknownChoice {
        /// The column.
        column: String,
        /// The value.
        value: String,
    },
    /// A tag or record a filter names by name or `human_id` does not exist.
    #[error("the `{column}` filter names `{value}`, which does not exist")]
    UnknownReference {
        /// The column.
        column: String,
        /// The name or `human_id` given.
        value: String,
    },
    /// The column cannot be sorted by (tags).
    #[error("the `{column}` column cannot be sorted")]
    Unsortable {
        /// The column.
        column: String,
    },
    /// The query lists another kind than the list it was run for.
    #[error("a {} query cannot list {} records", found.as_str(), expected.as_str())]
    KindMismatch {
        /// The kind the list shows.
        expected: ListedKind,
        /// The kind the query names.
        found: ListedKind,
    },
}

/// One page of a list and how many records the query matches in all.
#[derive(Debug, Clone, PartialEq)]
pub struct ListedPage<T> {
    /// The page's records, in the query's order.
    pub items: Vec<T>,
    /// Every record the query matches, the page aside.
    pub total: u64,
    /// Each item's cells for the query's columns, in their order, keyed by the item's `human_id` (a
    /// tag, which has none, by its id). A frontend reads a row's chosen columns here (ADR 0052 §7).
    pub cells: HashMap<String, Vec<Option<ListCell>>>,
}

/// Evaluates `query` over the store: the rows in `window`, each with typed cells for the query's
/// columns, and the total match count (ADR 0052 §4).
///
/// # Errors
///
/// [`AppError::ListQuery`] for a query the catalogue cannot evaluate, or a store error.
pub async fn query_list(workspace: &Workspace, query: &ListQuery, window: ListWindow) -> Result<ListResult, AppError> {
    let spec = spec(query, window)?;
    Ok(workspace.store().list_query(&spec).await?)
}

/// The id a filter on `kind`'s tag or record `column` holds for `reference` — a tag by its name (any
/// case) or id, a record by its `human_id` or id — so a frontend can take what a person types and the
/// query still holds ids (ADR 0052 §3).
///
/// # Errors
///
/// [`ListQueryError::UnknownReference`] when nothing has that name or id, any other
/// [`AppError::ListQuery`] for a column that holds no reference, or a store error.
pub async fn reference_id(
    workspace: &Workspace,
    kind: ListedKind,
    column: &str,
    reference: &str,
) -> Result<String, AppError> {
    let unknown = || ListQueryError::UnknownReference {
        column: column.to_owned(),
        value: reference.to_owned(),
    };
    let ty = kind
        .column(column)
        .ok_or_else(|| ListQueryError::UnknownColumn {
            kind,
            column: column.to_owned(),
        })?
        .ty;
    let store = workspace.store();
    let found = match (ty, record_target(kind, column)) {
        (ColumnType::Tags, _) => {
            let wanted = reference.trim().to_lowercase();
            let mut found = None;
            for tag in store.list_tags().await? {
                let id = tag.tag_id().map(|id| id.to_string());
                let named = tag.name().is_some_and(|name| name.to_lowercase() == wanted);
                if named || id.as_deref() == Some(reference.trim()) {
                    found = id;
                    break;
                }
            }
            found
        }
        (ColumnType::Record, Some(ListedKind::Person)) => store
            .find_person(reference)
            .await?
            .and_then(|view| view.person_id())
            .map(|id| id.to_string()),
        (ColumnType::Record, Some(ListedKind::Place)) => store
            .find_place(reference)
            .await?
            .and_then(|view| view.place_id())
            .map(|id| id.to_string()),
        (ColumnType::Record, Some(ListedKind::Source)) => store
            .find_source(reference)
            .await?
            .and_then(|view| view.source_id())
            .map(|id| id.to_string()),
        (ColumnType::Record, Some(ListedKind::DnaTest)) => store
            .find_dna_test(reference)
            .await?
            .and_then(|view| view.dna_test_id())
            .map(|id| id.to_string()),
        _ => return Err(unknown().into()),
    };
    Ok(found
        .or_else(|| uuid::Uuid::parse_str(reference).ok().map(|id| id.to_string()))
        .ok_or_else(unknown)?)
}

/// The kind a record column of `kind` refers to; `None` for any other column.
#[must_use]
pub const fn record_target(kind: ListedKind, column: &str) -> Option<ListedKind> {
    match (kind, column.as_bytes()) {
        (ListedKind::Event, b"place") | (ListedKind::Place, b"enclosed-by") => Some(ListedKind::Place),
        (ListedKind::Citation, b"source") => Some(ListedKind::Source),
        (ListedKind::DnaTest, b"person") => Some(ListedKind::Person),
        (ListedKind::DnaMatch, b"test-a" | b"test-b") => Some(ListedKind::DnaTest),
        _ => None,
    }
}

/// The records a paged list use-case reads: a window of the whole kind, or the ids a query picked.
pub(crate) enum Selection {
    /// A window over the kind in `human_id` order.
    Page(ListPage),
    /// These records, in this order.
    Ids(Vec<String>),
}

impl Selection {
    /// Every listed record.
    pub(crate) const ALL: Self = Self::Page(ListPage::ALL);
}

/// What a paged list use-case reads for a query: the records it picked, their total, and their cells.
pub(crate) struct Selected {
    /// The records picked, in the query's order.
    pub(crate) selection: Selection,
    /// Every record the query matches.
    total: u64,
    /// Each picked row's cells, by its list key (see [`ListedPage::cells`]).
    cells: HashMap<String, Vec<Option<ListCell>>>,
}

impl Selected {
    /// The page of `items`, read for this selection.
    pub(crate) fn into_page<T>(self, items: Vec<T>) -> ListedPage<T> {
        ListedPage {
            items,
            total: self.total,
            cells: self.cells,
        }
    }
}

/// The records `query` lists in `page`, their total and their cells — for a use-case listing `kind`.
///
/// # Errors
///
/// [`ListQueryError::KindMismatch`] for a query of another kind, any other [`AppError::ListQuery`],
/// or a store error.
pub(crate) async fn select(
    workspace: &Workspace,
    query: &ListQuery,
    kind: ListedKind,
    page: ListPage,
) -> Result<Selected, AppError> {
    if query.kind != kind {
        return Err(ListQueryError::KindMismatch {
            expected: kind,
            found: query.kind,
        }
        .into());
    }
    let window = ListWindow::Offset {
        offset: page.offset,
        limit: page.limit,
    };
    let result = query_list(workspace, query, window).await?;
    let mut ids = Vec::with_capacity(result.rows.len());
    let mut cells = HashMap::with_capacity(result.rows.len());
    for row in result.rows {
        cells.insert(row.human_id.unwrap_or_else(|| row.id.clone()), row.cells);
        ids.push(row.id);
    }
    Ok(Selected {
        selection: Selection::Ids(ids),
        total: result.total.unwrap_or_default(),
        cells,
    })
}

/// `views` in the order of `ids`, the ones not named dropped.
pub(crate) fn in_order<V>(views: Vec<V>, ids: &[String], id: impl Fn(&V) -> Option<String>) -> Vec<V> {
    let mut keyed: std::collections::HashMap<String, V> = views
        .into_iter()
        .filter_map(|view| id(&view).map(|key| (key, view)))
        .collect();
    let mut ordered = Vec::with_capacity(ids.len());
    for id in ids {
        if let Some(view) = keyed.remove(id) {
            ordered.push(view);
        }
    }
    ordered
}

/// The store's form of `query`, checked against its kind's catalogue. A tag or record id naming nothing
/// needs no check: no row links to it, so it matches nothing, and *has none of* it keeps everyone
/// (ADR 0052 §10).
fn spec(query: &ListQuery, window: ListWindow) -> Result<ListSpec, ListQueryError> {
    let kind = query.kind;
    let column = |id: &str| {
        kind.column(id).ok_or_else(|| ListQueryError::UnknownColumn {
            kind,
            column: id.to_owned(),
        })
    };
    let mut filters = Vec::with_capacity(query.filters.len());
    for filter in &query.filters {
        let ty = column(&filter.column)?.ty;
        filters.push(ListPredicate {
            column: filter.column.clone(),
            test: predicate(kind, filter, ty)?,
        });
    }
    let sort = match &query.sort {
        Some(sort) => {
            if column(&sort.column)?.ty == ColumnType::Tags {
                return Err(ListQueryError::Unsortable {
                    column: sort.column.clone(),
                });
            }
            Some(vitni_db::ListSort {
                column: sort.column.clone(),
                direction: sort.direction,
            })
        }
        None => None,
    };
    for id in &query.columns {
        column(id)?;
    }
    let search = query.search.trim();
    Ok(ListSpec {
        kind,
        search: (!search.is_empty()).then(|| search.to_owned()),
        filters,
        sort,
        columns: query.columns.clone(),
        window,
    })
}

/// The store test one filter makes, if its column's type takes its operator and value.
fn predicate(kind: ListedKind, filter: &ListFilter, ty: ColumnType) -> Result<ListTest, ListQueryError> {
    let column = &filter.column;
    let op = filter.op;
    if !FilterOp::for_type(ty).contains(&op) {
        return Err(ListQueryError::UnsupportedOperator {
            column: column.clone(),
            op,
        });
    }
    let wrong = || ListQueryError::WrongValue {
        column: column.clone(),
        op,
    };
    let test = match (op, &filter.value) {
        (FilterOp::Contains, FilterValue::Text(text)) => ListTest::Contains(text.clone()),
        (FilterOp::Is, FilterValue::Text(text)) => ListTest::Is(text.clone()),
        (FilterOp::IsEmpty, FilterValue::None) => ListTest::IsEmpty,
        (FilterOp::Before, FilterValue::Date(date)) => ListTest::Before(period(date).ok_or_else(wrong)?.0),
        (FilterOp::After, FilterValue::Date(date)) => ListTest::After(period(date).ok_or_else(wrong)?.1),
        (FilterOp::Between, FilterValue::DateRange { from, to }) if ty == ColumnType::Date => {
            let start = period(from).ok_or_else(wrong)?.0;
            let end = period(to).ok_or_else(wrong)?.1;
            ListTest::DatesBetween(start, end)
        }
        (FilterOp::LessThan, FilterValue::Number(number)) => ListTest::LessThan(*number),
        (FilterOp::GreaterThan, FilterValue::Number(number)) => ListTest::GreaterThan(*number),
        (FilterOp::Between, FilterValue::NumberRange { from, to }) if ty == ColumnType::Number => {
            ListTest::NumbersBetween(*from, *to)
        }
        (FilterOp::IsAnyOf, FilterValue::Values(values)) => ListTest::AnyOf(choices(kind, column, values)?),
        (FilterOp::IsNoneOf, FilterValue::Values(values)) => ListTest::NoneOf(choices(kind, column, values)?),
        (FilterOp::HasAnyOf, FilterValue::Values(ids)) => ListTest::AnyOf(ids.clone()),
        (FilterOp::HasAllOf, FilterValue::Values(ids)) => ListTest::AllOf(ids.clone()),
        (FilterOp::HasNoneOf, FilterValue::Values(ids)) => ListTest::NoneOf(ids.clone()),
        _ => return Err(wrong()),
    };
    Ok(test)
}

/// A date's period as sort keys: from its first day to its last (`1790` runs from 1790-00-00 to
/// 1790-12-31; a range to its end's last day). `None` for a text-only date, which has no place in the
/// order.
fn period(date: &GenealogicalDate) -> Option<(i64, i64)> {
    let GenealogicalDateBody::Structured(modifier) = &date.modifier else {
        return None;
    };
    let (start, end) = match modifier {
        DateModifier::None(point)
        | DateModifier::Before(point)
        | DateModifier::After(point)
        | DateModifier::About(point)
        | DateModifier::From(point)
        | DateModifier::To(point)
        | DateModifier::Interpreted { date: point, .. } => (point, point),
        DateModifier::Range { start, end } | DateModifier::Span { start, end } => (start, end),
    };
    Some((key(start, 0, 0), key(end, 12, 31)))
}

/// A point's sort key, its missing month and day read as `month` and `day`.
fn key(point: &DatePoint, month: u8, day: u8) -> i64 {
    let year = i64::from(point.year.unwrap_or(0));
    let month = i64::from(point.month.unwrap_or(month));
    let day = i64::from(point.day.unwrap_or(day));
    year * 10_000 + month * 100 + day
}

/// `values` as the column's choice keys (`Female` and `female` both `female`), each checked against the
/// column's enum.
fn choices(kind: ListedKind, column: &str, values: &[String]) -> Result<Vec<String>, ListQueryError> {
    let mut keys = Vec::with_capacity(values.len());
    for value in values {
        let key = choice_name(value.trim());
        if !is_choice(kind, column, &key) {
            return Err(ListQueryError::UnknownChoice {
                column: column.to_owned(),
                value: value.clone(),
            });
        }
        keys.push(key);
    }
    Ok(keys)
}

/// Whether `key` names a variant of the enum behind `kind`'s `column`.
fn is_choice(kind: ListedKind, column: &str, key: &str) -> bool {
    match (kind, column) {
        (ListedKind::Person, "sex") => names::<Sex>(key),
        (ListedKind::Event, "type") => names::<EventType>(key),
        (ListedKind::Place, "type") => names::<PlaceType>(key),
        (ListedKind::Citation, "confidence") => names::<Confidence>(key),
        (ListedKind::Repository, "type") => names::<RepositoryType>(key),
        (ListedKind::Note, "type") => names::<NoteType>(key),
        (ListedKind::DnaTest, "provider") => names::<DnaProvider>(key),
        (ListedKind::DnaTest, "test-type") => names::<DnaTestType>(key),
        (ListedKind::DnaMatch, "status") => names::<MatchStatus>(key),
        _ => false,
    }
}

/// Whether `T` has a unit variant named `key` (kebab-case): it deserializes from its `PascalCase`
/// name, plain or as an adjacently tagged `{"type": …}`. A variant carrying a value (`Custom`) does not.
fn names<T: serde::de::DeserializeOwned>(key: &str) -> bool {
    let name = pascal(key);
    serde_json::from_value::<T>(serde_json::Value::String(name.clone())).is_ok()
        || serde_json::from_value::<T>(serde_json::json!({ "type": name })).is_ok()
}

/// `kebab-case` → `PascalCase`.
fn pascal(key: &str) -> String {
    let mut pascal = String::with_capacity(key.len());
    for word in key.split('-') {
        let mut characters = word.chars();
        if let Some(first) = characters.next() {
            pascal.extend(first.to_uppercase());
            pascal.extend(characters);
        }
    }
    pascal
}

#[cfg(test)]
mod tests {
    use vitni_db::ListedKind;

    use vitni_db::choice_name;

    use super::{FilterOp, is_choice, pascal};

    #[test]
    fn every_operator_round_trips_its_name() {
        for op in FilterOp::ALL {
            assert_eq!(FilterOp::parse(op.as_str()), Some(*op));
            let json = serde_json::to_value(op).unwrap_or_default();
            assert_eq!(json, serde_json::Value::String(op.as_str().to_owned()));
        }
    }

    #[test]
    fn a_choice_is_named_in_kebab_case_whatever_its_spelling() {
        assert_eq!(choice_name("VeryHigh"), "very-high");
        assert_eq!(choice_name("very-high"), "very-high");
        assert_eq!(pascal("twenty-three-and-me"), "TwentyThreeAndMe");
        assert!(is_choice(ListedKind::Person, "sex", "female"));
        assert!(is_choice(ListedKind::Citation, "confidence", "very-high"));
        assert!(is_choice(ListedKind::DnaTest, "provider", "twenty-three-and-me"));
        assert!(
            !is_choice(ListedKind::Person, "sex", "other"),
            "an escape variant carries a value"
        );
        assert!(!is_choice(ListedKind::Person, "sex", "femal"));
    }

    #[test]
    fn every_choice_column_has_its_enum() {
        for kind in ListedKind::ALL {
            for column in kind.columns() {
                if column.ty == vitni_db::ColumnType::Choice {
                    let some = [
                        "male",
                        "birth",
                        "parish",
                        "high",
                        "library",
                        "general",
                        "ancestry-dna",
                        "autosomal",
                        "confirmed",
                    ];
                    assert!(
                        some.iter().any(|key| is_choice(*kind, column.id, key)),
                        "{kind:?} {}",
                        column.id
                    );
                }
            }
        }
    }
}
