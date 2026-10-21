//! Saved searches (ADR 0052 §8-10): named list queries stored in `workspace.toml`.
//!
//! The manifest holds each entry as a raw TOML table so one entry this version cannot read never
//! blocks the others, and is written back with the same keys and values (the TOML is re-serialized, so
//! comments and key order are not kept). An entry with a field this version
//! does not know is unreadable too, so a rewrite never strips a field a newer version added, and so is
//! one that repeats the id of an earlier entry, so it cannot take the first one with it.

use serde::{Deserialize, Serialize};
use uuid::Uuid;
use vitni_db::ListedKind;

use crate::config_store::ConfigStore;
use crate::error::AppError;
use crate::list_query::{
    FilterOp, ListFilter, ListQuery, ListQueryError, ListSort, check_filter, sortable, validate_query,
};
use crate::session::Session;

/// The identity of a saved search (UUID v7, minted by the `Session`).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(transparent)]
pub struct SavedSearchId(Uuid);

impl SavedSearchId {
    /// Wraps an id minted by the caller.
    #[must_use]
    pub const fn from_uuid(id: Uuid) -> Self {
        Self(id)
    }
}

impl std::fmt::Display for SavedSearchId {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        self.0.fmt(f)
    }
}

/// The query a saved search stores. Clauses and the sort stay raw TOML so a clause on a column or
/// operator this version does not know still loads and is written back unchanged (ADR 0052 §10).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StoredQuery {
    /// The kind of record the query lists.
    pub kind: ListedKind,
    /// The free-text search.
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub search: String,
    /// The filter clauses, as stored.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub filters: Vec<toml::Value>,
    /// The sort, as stored.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sort: Option<toml::Value>,
    /// The column ids to show; absent means the kind's default columns, empty means none.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub columns: Option<Vec<String>>,
}

/// A named, stored query.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedSearch {
    /// The saved search's identity.
    pub id: SavedSearchId,
    /// The user-given name, unique per workspace ignoring case.
    pub name: String,
    /// The stored query.
    pub query: StoredQuery,
}

/// One `[[saved_searches]]` entry as loaded: understood, or kept raw so a rewrite loses nothing.
#[derive(Debug, Clone, PartialEq)]
pub enum SavedSearchEntry {
    /// An entry this version understands.
    Readable(SavedSearch),
    /// An entry this version cannot use, with the original table: one that cannot be parsed, carries a
    /// field this version does not know, or repeats the id of an earlier entry.
    Unreadable {
        /// The table exactly as found.
        raw: toml::Value,
    },
}

impl SavedSearchEntry {
    /// Interprets the entry at zero-based `position` in `[[saved_searches]]`.
    #[must_use]
    pub fn from_value(position: usize, raw: &toml::Value) -> Self {
        match raw.clone().try_into::<SavedSearch>() {
            Ok(search) => Self::Readable(search),
            Err(error) => {
                tracing::warn!(position, %error, "saved search is unreadable and is kept unchanged");
                Self::Unreadable { raw: raw.clone() }
            }
        }
    }

    /// The TOML table to store for this entry; an unreadable entry is returned as found.
    #[must_use]
    pub fn to_value(&self) -> toml::Value {
        match self {
            Self::Readable(search) => toml::Value::try_from(search).unwrap_or_else(|error| {
                // Unreachable: every field is a string, a uuid, a unit enum or already TOML.
                tracing::error!(%error, id = ?search.id, "saved search could not be serialised");
                toml::Value::Table(toml::Table::new())
            }),
            Self::Unreadable { raw } => raw.clone(),
        }
    }
}
/// Why a saved-search change was refused.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
pub enum SavedSearchError {
    /// The name is empty or only whitespace.
    #[error("a saved search needs a name")]
    EmptyName,
    /// Another saved search already has the name, ignoring case.
    #[error("a saved search named `{name}` already exists")]
    DuplicateName {
        /// The name refused, trimmed.
        name: String,
    },
    /// No saved search has the name looked up.
    #[error("no saved search `{reference}`")]
    NotFound {
        /// The name looked up.
        reference: String,
    },
    /// The saved search an id points to is gone, for example deleted from another frontend.
    #[error("the saved search no longer exists")]
    Removed,
    /// The saved search has clauses the catalogue cannot take, so it does not run (ADR 0052 §10).
    #[error("saved search `{name}` cannot run: {} stale clause(s)", clauses.len())]
    Broken {
        /// The saved search's name.
        name: String,
        /// The clauses that cannot run.
        clauses: Vec<StaleClause>,
    },
}

/// A readable saved search as listed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SavedSearchSummary {
    /// The saved search's identity.
    pub id: SavedSearchId,
    /// Its name.
    pub name: String,
    /// The kind of record it lists.
    pub kind: ListedKind,
}

/// An entry this version cannot use (unparseable, carrying an unknown field, or repeating an earlier
/// entry's id), reported so a frontend can say so; it stays on disk unchanged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnreadableSavedSearch {
    /// Zero-based position in `[[saved_searches]]`.
    pub position: usize,
    /// The entry's name, when it has a readable one.
    pub name: Option<String>,
}

/// The workspace's saved searches: the readable ones and the entries set aside.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct SavedSearchList {
    /// The readable saved searches, in stored order.
    pub searches: Vec<SavedSearchSummary>,
    /// The entries this version cannot read.
    pub unreadable: Vec<UnreadableSavedSearch>,
}

fn raw_name(raw: &toml::Value) -> Option<String> {
    raw.get("name").and_then(toml::Value::as_str).map(str::to_owned)
}

fn to_toml(value: &impl Serialize) -> Result<toml::Value, AppError> {
    toml::Value::try_from(value).map_err(|error| AppError::Config(format!("saved search query: {error}")))
}

fn stored_query(query: &ListQuery) -> Result<StoredQuery, AppError> {
    validate_query(query)?;
    let mut filters = Vec::with_capacity(query.filters.len());
    for filter in &query.filters {
        filters.push(to_toml(filter)?);
    }
    let sort = query.sort.as_ref().map(to_toml).transpose()?;
    Ok(StoredQuery {
        kind: query.kind,
        search: query.search.clone(),
        filters,
        sort,
        columns: query.columns.clone(),
    })
}

/// Whether two saved-search names are the same name: equal ignoring case and surrounding space.
#[must_use]
pub fn same_saved_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
}

/// The trimmed `name`, refused when blank or when another entry (readable or not) already has it,
/// ignoring case. `own` is the entry being renamed, which may keep its name in another case.
fn checked_name(
    entries: &[SavedSearchEntry],
    name: &str,
    own: Option<SavedSearchId>,
) -> Result<String, SavedSearchError> {
    let name = name.trim();
    if name.is_empty() {
        return Err(SavedSearchError::EmptyName);
    }
    for entry in entries {
        let taken = match entry {
            SavedSearchEntry::Readable(search) if Some(search.id) == own => continue,
            SavedSearchEntry::Readable(search) => Some(search.name.clone()),
            SavedSearchEntry::Unreadable { raw } => raw_name(raw),
        };
        if taken.is_some_and(|taken| same_saved_name(&taken, name)) {
            return Err(SavedSearchError::DuplicateName { name: name.to_owned() });
        }
    }
    Ok(name.to_owned())
}

fn find_mut(entries: &mut [SavedSearchEntry], id: SavedSearchId) -> Result<&mut SavedSearch, SavedSearchError> {
    for entry in entries {
        if let SavedSearchEntry::Readable(search) = entry
            && search.id == id
        {
            return Ok(search);
        }
    }
    Err(SavedSearchError::Removed)
}

/// The saved searches, with the entries this version cannot read set aside.
///
/// # Errors
///
/// [`AppError`] when the manifest cannot be read.
pub fn list_saved_searches(store: &impl ConfigStore) -> Result<SavedSearchList, AppError> {
    let mut list = SavedSearchList::default();
    for (position, entry) in store.load_saved_searches()?.into_iter().enumerate() {
        match entry {
            SavedSearchEntry::Readable(search) => list.searches.push(SavedSearchSummary {
                id: search.id,
                name: search.name,
                kind: search.query.kind,
            }),
            SavedSearchEntry::Unreadable { raw } => list.unreadable.push(UnreadableSavedSearch {
                position,
                name: raw_name(&raw),
            }),
        }
    }
    Ok(list)
}

/// Saves `query` as a new saved search called `name`, with an id minted by `session`.
///
/// # Errors
///
/// [`SavedSearchError::EmptyName`] or [`SavedSearchError::DuplicateName`] for a refused name,
/// [`AppError::ListQuery`] for a query the catalogue cannot evaluate, or a manifest error.
pub fn save_saved_search(
    store: &impl ConfigStore,
    session: &Session,
    name: &str,
    query: &ListQuery,
) -> Result<SavedSearchId, AppError> {
    let mut entries = store.load_saved_searches()?;
    let name = checked_name(&entries, name, None)?;
    let query = stored_query(query)?;
    let id = session.new_saved_search_id();
    entries.push(SavedSearchEntry::Readable(SavedSearch { id, name, query }));
    store.store_saved_searches(&entries)?;
    Ok(id)
}

/// Replaces the query of saved search `id`, keeping its name.
///
/// # Errors
///
/// [`SavedSearchError::Removed`], [`AppError::ListQuery`] for a query the catalogue cannot
/// evaluate, or a manifest error.
pub fn update_saved_search(store: &impl ConfigStore, id: SavedSearchId, query: &ListQuery) -> Result<(), AppError> {
    let mut entries = store.load_saved_searches()?;
    let search = find_mut(&mut entries, id)?;
    search.query = stored_query(query)?;
    store.store_saved_searches(&entries)
}

/// Renames saved search `id` to `name`.
///
/// # Errors
///
/// [`SavedSearchError::Removed`], [`SavedSearchError::EmptyName`], [`SavedSearchError::DuplicateName`],
/// or a manifest error.
pub fn rename_saved_search(store: &impl ConfigStore, id: SavedSearchId, name: &str) -> Result<(), AppError> {
    let mut entries = store.load_saved_searches()?;
    find_mut(&mut entries, id)?;
    let name = checked_name(&entries, name, Some(id))?;
    find_mut(&mut entries, id)?.name = name;
    store.store_saved_searches(&entries)
}

/// Deletes saved search `id`.
///
/// # Errors
///
/// [`SavedSearchError::Removed`], or a manifest error.
pub fn delete_saved_search(store: &impl ConfigStore, id: SavedSearchId) -> Result<(), AppError> {
    let mut entries = store.load_saved_searches()?;
    let before = entries.len();
    entries.retain(|entry| match entry {
        SavedSearchEntry::Readable(search) => search.id != id,
        SavedSearchEntry::Unreadable { .. } => true,
    });
    if entries.len() == before {
        return Err(SavedSearchError::Removed.into());
    }
    store.store_saved_searches(&entries)
}

/// A filter clause that cannot run, by its zero-based position among the saved search's clauses.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct StaleClause {
    /// Position in the stored filters.
    pub index: usize,
    /// The column the clause names, when it names one.
    pub column: Option<String>,
    /// The operator the clause names, when it names one.
    pub op: Option<String>,
    /// Why the catalogue cannot take it.
    pub error: ListQueryError,
}

/// What a stale clause leaves out of the column and operator a clause needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ClauseGap {
    /// The clause names no column.
    Column,
    /// The clause names no operator.
    Operator,
    /// The clause names neither.
    Both,
}

impl StaleClause {
    /// What the clause leaves unnamed, or `None` when it names both a column and an operator.
    #[must_use]
    pub fn gap(&self) -> Option<ClauseGap> {
        match (&self.column, &self.op) {
            (Some(_), Some(_)) => None,
            (None, Some(_)) => Some(ClauseGap::Column),
            (Some(_), None) => Some(ClauseGap::Operator),
            (None, None) => Some(ClauseGap::Both),
        }
    }

    /// The clause's column and operator as stored, those it names joined by `separator`.
    #[must_use]
    pub fn named(&self, separator: &str) -> String {
        let parts: Vec<&str> = [self.column.as_deref(), self.op.as_deref()]
            .into_iter()
            .flatten()
            .collect();
        parts.join(separator)
    }
}

/// A saved search resolved against the current catalogue.
#[derive(Debug, Clone, PartialEq)]
pub struct SavedSearchResolution {
    /// The saved search as stored.
    pub search: SavedSearch,
    /// The query to run: the clauses that could be read, a sort and columns the catalogue still has.
    /// Run it only when [`Self::stale`] is empty.
    pub query: ListQuery,
    /// Clauses on an unknown column or operator, with a value the operator does not take or a choice it
    /// does not offer, or carrying a key this version does not model.
    pub stale: Vec<StaleClause>,
}

fn runnable_filter(kind: ListedKind, index: usize, raw: &toml::Value) -> Result<ListFilter, StaleClause> {
    let named = |key| raw.get(key).and_then(toml::Value::as_str);
    let column = named("column").unwrap_or_default().to_owned();
    let op = named("op").unwrap_or_default();
    let stale = |error| StaleClause {
        index,
        column: named("column").filter(|name| !name.is_empty()).map(str::to_owned),
        op: named("op").filter(|name| !name.is_empty()).map(str::to_owned),
        error,
    };
    let Ok(filter) = raw.clone().try_into::<ListFilter>() else {
        let error = match (kind.column(&column), FilterOp::parse(op)) {
            (None, _) => ListQueryError::UnknownColumn {
                kind,
                column: column.clone(),
            },
            (Some(_), None) => ListQueryError::UnknownOperator {
                column: column.clone(),
                op: op.to_owned(),
            },
            (Some(_), Some(op)) => ListQueryError::WrongValue {
                column: column.clone(),
                op,
            },
        };
        return Err(stale(error));
    };
    check_filter(kind, &filter).map_err(stale)?;
    Ok(filter)
}

/// The keys of a stored sort; a table carrying any other is not one this version can run.
const SORT_KEYS: [&str; 2] = ["column", "direction"];

fn query_of(stored: &StoredQuery) -> (ListQuery, Vec<StaleClause>) {
    let kind = stored.kind;
    let mut query = ListQuery::new(kind);
    query.search.clone_from(&stored.search);
    let mut stale = Vec::new();
    for (index, raw) in stored.filters.iter().enumerate() {
        match runnable_filter(kind, index, raw) {
            Ok(filter) => query.filters.push(filter),
            Err(clause) => stale.push(clause),
        }
    }
    let sort = stored
        .sort
        .as_ref()
        .filter(|raw| {
            raw.as_table()
                .is_some_and(|table| table.keys().all(|key| SORT_KEYS.contains(&key.as_str())))
        })
        .and_then(|raw| raw.clone().try_into::<ListSort>().ok());
    query.sort = sort.filter(|sort| sortable(kind, &sort.column).unwrap_or(false));
    query.columns = stored
        .columns
        .as_ref()
        .map(|ids| ids.iter().filter(|id| kind.column(id).is_some()).cloned().collect());
    (query, stale)
}

/// Finds the saved search called `reference` (any case) or having that id, and resolves it against the
/// current catalogue (ADR 0052 §10).
///
/// A tag or record id in a clause that no longer exists is kept as stored: it matches nothing, so
/// *has any of* and *is* narrow to nothing, *has none of* keeps everyone, and *has all of* matches
/// nobody. Ids are never pruned, since dropping one would let *has all of* widen to the others. A clause
/// on an unknown column or operator, with a value its operator does not take or an unknown choice, or
/// carrying a key this version does not model, is reported in [`SavedSearchResolution::stale`] and the
/// search must not run. A sort or display column the catalogue no longer has is dropped.
///
/// # Errors
///
/// [`SavedSearchError::NotFound`] when nothing has the name or id, [`ListQueryError::KindMismatch`]
/// when `kind` is given and the search lists another, or a manifest error.
pub fn resolve_saved_search(
    store: &impl ConfigStore,
    reference: &str,
    kind: Option<ListedKind>,
) -> Result<SavedSearchResolution, AppError> {
    let reference = reference.trim();
    let folded_id = reference.to_lowercase();
    resolve_matching(store, Some(reference), kind, |search| {
        same_saved_name(&search.name, reference) || search.id.to_string() == folded_id
    })
}

/// Resolves the saved search having `id`, as [`resolve_saved_search`] does for a name or id.
///
/// # Errors
///
/// [`SavedSearchError::Removed`] when no saved search has the id, otherwise those of [`resolve_saved_search`].
pub fn resolve_saved_search_id(
    store: &impl ConfigStore,
    id: SavedSearchId,
    kind: Option<ListedKind>,
) -> Result<SavedSearchResolution, AppError> {
    resolve_matching(store, None, kind, |search| search.id == id)
}

fn resolve_matching(
    store: &impl ConfigStore,
    reference: Option<&str>,
    kind: Option<ListedKind>,
    is_match: impl Fn(&SavedSearch) -> bool,
) -> Result<SavedSearchResolution, AppError> {
    let mut found = None;
    for entry in store.load_saved_searches()? {
        if let SavedSearchEntry::Readable(search) = entry
            && is_match(&search)
        {
            found = Some(search);
            break;
        }
    }
    let Some(search) = found else {
        return Err(match reference {
            Some(reference) => SavedSearchError::NotFound {
                reference: reference.to_owned(),
            },
            None => SavedSearchError::Removed,
        }
        .into());
    };
    if let Some(expected) = kind
        && expected != search.query.kind
    {
        return Err(ListQueryError::KindMismatch {
            expected,
            found: search.query.kind,
        }
        .into());
    }
    let (query, stale) = query_of(&search.query);
    Ok(SavedSearchResolution { search, query, stale })
}

/// The runnable query of the saved search `reference`, which must list `kind` (ADR 0052 §10).
///
/// # Errors
///
/// Those of [`resolve_saved_search`], and [`SavedSearchError::Broken`] when any clause is stale, since a
/// search with stale clauses does not run.
pub fn runnable_saved_query(
    store: &impl ConfigStore,
    reference: &str,
    kind: ListedKind,
) -> Result<ListQuery, AppError> {
    let resolution = resolve_saved_search(store, reference, Some(kind))?;
    if !resolution.stale.is_empty() {
        return Err(SavedSearchError::Broken {
            name: resolution.search.name,
            clauses: resolution.stale,
        }
        .into());
    }
    Ok(resolution.query)
}

#[cfg(test)]
mod tests {
    use super::{SavedSearch, SavedSearchEntry, SavedSearchId, StoredQuery};
    use crate::list_query::{FilterOp, FilterValue, ListFilter, ListSort};
    use uuid::Uuid;
    use vitni_core::date::{Calendar, DateModifier, DatePoint, DateQuality, GenealogicalDate, GenealogicalDateBody};
    use vitni_db::{ListedKind, SortDirection};

    fn date(year: i32, month: Option<u8>) -> GenealogicalDate {
        GenealogicalDate {
            calendar: Calendar::Gregorian,
            quality: DateQuality::Estimated,
            modifier: GenealogicalDateBody::Structured(DateModifier::Range {
                start: DatePoint {
                    year: Some(year),
                    month,
                    day: None,
                },
                end: DatePoint {
                    year: Some(year + 1),
                    month: None,
                    day: None,
                },
            }),
            time: None,
            new_year_begins: None,
            sort_value: i64::from(year) * 10_000,
            original_text: Some(format!("c. {year}")),
        }
    }

    fn clause(column: &str, op: FilterOp, value: FilterValue) -> toml::Value {
        toml::Value::try_from(ListFilter {
            column: column.to_owned(),
            op,
            value,
        })
        .expect("clause as toml")
    }

    fn every_variant_search() -> SavedSearch {
        SavedSearch {
            id: SavedSearchId::from_uuid(Uuid::from_u128(0x0190_0000_0000_7000_8000_0000_0000_0001)),
            name: "Everything".to_owned(),
            query: StoredQuery {
                kind: ListedKind::Person,
                search: "ada".to_owned(),
                filters: vec![
                    clause("birth", FilterOp::IsEmpty, FilterValue::None),
                    clause("name", FilterOp::Contains, FilterValue::Text("Lovelace".to_owned())),
                    clause("birth", FilterOp::Before, FilterValue::Date(date(1815, Some(12)))),
                    clause(
                        "birth",
                        FilterOp::Between,
                        FilterValue::DateRange {
                            from: date(1800, None),
                            to: date(1850, Some(6)),
                        },
                    ),
                    clause("children", FilterOp::GreaterThan, FilterValue::Number(2.5)),
                    clause(
                        "children",
                        FilterOp::Between,
                        FilterValue::NumberRange { from: 1.0, to: 4.0 },
                    ),
                    clause(
                        "tags",
                        FilterOp::HasAllOf,
                        FilterValue::Values(vec!["t1".to_owned(), "t2".to_owned()]),
                    ),
                    clause(
                        "father",
                        FilterOp::Is,
                        FilterValue::Text("0190aaaa-0000-7000-8000-000000000002".to_owned()),
                    ),
                ],
                sort: Some(
                    toml::Value::try_from(ListSort {
                        column: "birth".to_owned(),
                        direction: SortDirection::Descending,
                    })
                    .expect("sort as toml"),
                ),
                columns: Some(vec!["name".to_owned(), "birth".to_owned()]),
            },
        }
    }

    #[test]
    fn a_readable_entry_round_trips_every_filter_value_variant() {
        let search = every_variant_search();
        let value = SavedSearchEntry::Readable(search.clone()).to_value();
        let text = toml::to_string(&value).expect("serialise");
        let reparsed: toml::Value = toml::from_str(&text).expect("reparse");

        assert_eq!(
            SavedSearchEntry::from_value(0, &reparsed),
            SavedSearchEntry::Readable(search)
        );
    }

    #[test]
    fn a_clause_with_an_unknown_column_operator_or_value_still_loads_and_round_trips_unchanged() {
        let odd_clauses: Vec<toml::Value> = [
            r#"column = "zodiac"
op = "contains"
value = { type = "text", value = "Leo" }"#,
            r#"column = "name"
op = "sounds-like"
value = { type = "text", value = "Ada" }"#,
            r#"column = "birth"
op = "before"
value = { type = "hologram", value = 7 }"#,
        ]
        .iter()
        .map(|text| toml::from_str(text).expect("clause fixture"))
        .collect();
        let mut search = every_variant_search();
        search.query.filters = odd_clauses.clone();

        let value = SavedSearchEntry::Readable(search).to_value();
        let SavedSearchEntry::Readable(loaded) = SavedSearchEntry::from_value(0, &value) else {
            panic!("an entry with odd clauses must stay readable");
        };

        assert_eq!(loaded.query.filters, odd_clauses);
        assert_eq!(SavedSearchEntry::Readable(loaded).to_value(), value);
    }

    #[test]
    fn a_clause_with_a_key_this_version_does_not_know_is_stale() {
        for text in [
            "column = \"surname\"\nop = \"contains\"\nnegate = true\nvalue = { type = \"text\", value = \"ols\" }",
            "column = \"surname\"\nop = \"contains\"\nvalue = { type = \"text\", value = \"ols\", fold = true }",
        ] {
            let raw: toml::Value = toml::from_str(text).expect("clause fixture");

            let result = super::runnable_filter(ListedKind::Person, 0, &raw);

            assert!(result.is_err(), "{text}: {result:?}");
        }
    }

    #[test]
    fn an_entry_missing_its_id_is_unreadable_and_kept_unchanged() {
        let raw: toml::Value = toml::from_str("name = \"No id\"\n[query]\nkind = \"person\"\n").expect("fixture");

        let entry = SavedSearchEntry::from_value(3, &raw);

        let SavedSearchEntry::Unreadable { raw: kept } = &entry else {
            panic!("expected Unreadable, got {entry:?}");
        };
        assert_eq!(kept, &raw);
        assert_eq!(entry.to_value(), raw);
    }

    #[test]
    fn an_entry_with_a_field_this_version_does_not_know_is_unreadable_and_kept_unchanged() {
        for text in [
            "id = \"0190aaaa-0000-7000-8000-000000000001\"\nname = \"X\"\n[query]\nkind = \"person\"\n\
             dataset = \"gedcom:1\"\n",
            "id = \"0190aaaa-0000-7000-8000-000000000001\"\nname = \"X\"\nowner = \"me\"\n[query]\nkind = \"person\"\n",
        ] {
            let raw: toml::Value = toml::from_str(text).expect("fixture");

            let entry = SavedSearchEntry::from_value(0, &raw);

            assert!(
                matches!(entry, SavedSearchEntry::Unreadable { .. }),
                "{text}: {entry:?}"
            );
            assert_eq!(entry.to_value(), raw);
        }
    }

    #[test]
    fn an_entry_with_a_non_string_name_is_unreadable() {
        let raw: toml::Value =
            toml::from_str("id = \"0190aaaa-0000-7000-8000-000000000001\"\nname = 42\n[query]\nkind = \"person\"\n")
                .expect("fixture");

        let entry = SavedSearchEntry::from_value(0, &raw);

        assert!(matches!(entry, SavedSearchEntry::Unreadable { .. }), "{entry:?}");
        assert_eq!(entry.to_value(), raw);
    }

    #[test]
    fn a_key_added_inside_a_range_value_makes_the_clause_stale() {
        let raw: toml::Value = toml::from_str(
            "column = \"children\"\nop = \"between\"\n\
             value = { type = \"number-range\", value = { from = 1.0, to = 3.0, exclusive = true } }",
        )
        .expect("fixture");
        assert!(super::runnable_filter(ListedKind::Family, 0, &raw).is_err());
    }

    #[test]
    fn an_entry_with_an_unknown_kind_is_unreadable() {
        let raw: toml::Value = toml::from_str(
            "id = \"0190aaaa-0000-7000-8000-000000000001\"\nname = \"Galaxies\"\n[query]\nkind = \"galaxy\"\n",
        )
        .expect("fixture");

        let entry = SavedSearchEntry::from_value(1, &raw);

        assert!(matches!(entry, SavedSearchEntry::Unreadable { .. }), "{entry:?}");
        assert_eq!(entry.to_value(), raw);
    }

    #[test]
    fn a_stale_clause_names_the_column_and_operator_it_has_in_that_order() {
        let clause = |column: Option<&str>, op: Option<&str>| super::StaleClause {
            index: 0,
            column: column.map(str::to_owned),
            op: op.map(str::to_owned),
            error: crate::list_query::ListQueryError::UnknownColumn {
                kind: ListedKind::Person,
                column: "x".to_owned(),
            },
        };

        assert_eq!(clause(Some("zodiac"), Some("is")).named(":"), "zodiac:is");
        assert_eq!(clause(Some("zodiac"), Some("is")).named(" "), "zodiac is");
        assert_eq!(clause(Some("zodiac"), None).named(":"), "zodiac");
        assert_eq!(clause(None, Some("is")).named(":"), "is");
        assert_eq!(clause(None, None).named(":"), "");
        assert_eq!(clause(None, None).gap(), Some(super::ClauseGap::Both));
        assert_eq!(clause(None, Some("is")).gap(), Some(super::ClauseGap::Column));
        assert_eq!(clause(Some("zodiac"), None).gap(), Some(super::ClauseGap::Operator));
        assert_eq!(clause(Some("zodiac"), Some("is")).gap(), None);
    }
}
