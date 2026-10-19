//! The list tables' SQL, written once for both engines: their DDL, the row upsert, and the list query
//! (ADR 0052 §4). Only placeholders, column types, substring search and text collation differ, and
//! [`Dialect`] spells those.

use vitni_core::date::{GenealogicalDate, GenealogicalDateBody};

use super::rows::RowValues;
use super::{ColumnType, ListCell, ListColumn, ListSpec, ListTest, ListWindow, ListedKind, SortDirection, SortKey};
use crate::store::DbError;

/// The engine a statement is written for.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum Dialect {
    Sqlite,
    Postgres,
}

impl Dialect {
    const fn integer(self) -> &'static str {
        match self {
            Self::Sqlite => "INTEGER",
            Self::Postgres => "BIGINT",
        }
    }

    const fn real(self) -> &'static str {
        match self {
            Self::Sqlite => "REAL",
            Self::Postgres => "DOUBLE PRECISION",
        }
    }

    /// `haystack` contains `needle`.
    fn contains(self, haystack: &str, needle: &str) -> String {
        match self {
            Self::Sqlite => format!("instr({haystack}, {needle}) > 0"),
            Self::Postgres => format!("strpos({haystack}, {needle}) > 0"),
        }
    }

    /// `expression` compared bytewise, so the order and the keyset comparisons agree on both engines.
    fn bytewise(self, expression: &str) -> String {
        match self {
            Self::Sqlite => expression.to_owned(),
            Self::Postgres => format!("{expression} COLLATE \"C\""),
        }
    }

    /// The record's tag ids, comma-joined.
    const fn tag_list(self) -> &'static str {
        match self {
            Self::Sqlite => {
                "(SELECT group_concat(l.target, ',') FROM record_links l \
                 WHERE l.relation = 'tagged' AND l.source = t.view_id)"
            }
            Self::Postgres => {
                "(SELECT string_agg(l.target, ',') FROM record_links l \
                 WHERE l.relation = 'tagged' AND l.source = t.view_id)"
            }
        }
    }
}

/// A bound value.
#[derive(Debug, Clone, PartialEq)]
pub(crate) enum Value {
    Text(Option<String>),
    Int(Option<i64>),
    Real(Option<f64>),
}

/// A statement and its bound values, in placeholder order.
#[derive(Debug)]
pub(crate) struct Statement {
    pub(crate) sql: String,
    pub(crate) binds: Vec<Value>,
}

/// Collects bound values, handing out each one's placeholder.
struct Binder {
    dialect: Dialect,
    binds: Vec<Value>,
}

impl Binder {
    const fn new(dialect: Dialect) -> Self {
        Self {
            dialect,
            binds: Vec::new(),
        }
    }

    fn bind(&mut self, value: Value) -> String {
        self.binds.push(value);
        match self.dialect {
            Dialect::Sqlite => "?".to_owned(),
            Dialect::Postgres => format!("${}", self.binds.len()),
        }
    }

    fn text(&mut self, value: &str) -> String {
        self.bind(Value::Text(Some(value.to_owned())))
    }

    fn list(&mut self, values: &[String]) -> String {
        let mut placeholders = Vec::with_capacity(values.len());
        for value in values {
            placeholders.push(self.text(value));
        }
        placeholders.join(", ")
    }

    fn finish(self, sql: String) -> Statement {
        Statement { sql, binds: self.binds }
    }
}

/// Case folding, applied in Rust so it reaches beyond ASCII alike on both engines.
pub(crate) fn fold(text: &str) -> String {
    text.to_lowercase()
}

/// The column's SQL name stem: `enclosed-by` → `c_enclosed_by`.
fn stem(column: &ListColumn) -> String {
    format!("c_{}", column.id.replace('-', "_"))
}

/// The SQL type of a stored value.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stored {
    Text,
    Int,
    Real,
}

impl Stored {
    const fn sql(self, dialect: Dialect) -> &'static str {
        match self {
            Self::Text => "TEXT",
            Self::Int => dialect.integer(),
            Self::Real => dialect.real(),
        }
    }

    /// A NULL of this type: Postgres refuses a text NULL for a number column.
    const fn null(self) -> Value {
        match self {
            Self::Text => Value::Text(None),
            Self::Int => Value::Int(None),
            Self::Real => Value::Real(None),
        }
    }
}

/// The SQL columns a catalogue column is stored in, with their types; none for the tag column.
fn physical(column: &ListColumn) -> Vec<(String, Stored)> {
    let stem = stem(column);
    match column.ty {
        ColumnType::Text => vec![(stem.clone(), Stored::Text), (format!("{stem}_fold"), Stored::Text)],
        ColumnType::Date => vec![(stem.clone(), Stored::Text), (format!("{stem}_key"), Stored::Int)],
        ColumnType::Number => vec![(stem, Stored::Real)],
        ColumnType::Choice => vec![(stem, Stored::Text)],
        ColumnType::Record => vec![
            (stem.clone(), Stored::Text),
            (format!("{stem}_title"), Stored::Text),
            (format!("{stem}_fold"), Stored::Text),
        ],
        ColumnType::Tags => Vec::new(),
    }
}

/// Every SQL column of `kind`'s list table and its declared type, in table order.
pub(crate) fn table_columns(dialect: Dialect, kind: ListedKind) -> Vec<(String, &'static str)> {
    let mut columns = vec![
        ("view_id".to_owned(), "TEXT NOT NULL PRIMARY KEY"),
        ("human_id".to_owned(), "TEXT"),
        ("search".to_owned(), "TEXT NOT NULL"),
    ];
    for column in kind.columns() {
        for (name, stored) in physical(column) {
            columns.push((name, stored.sql(dialect)));
        }
    }
    columns
}

/// The `CREATE TABLE` of `kind`'s list table.
pub(crate) fn create_table(dialect: Dialect, kind: ListedKind) -> String {
    let columns: Vec<String> = table_columns(dialect, kind)
        .into_iter()
        .map(|(name, ty)| format!("{name} {ty}"))
        .collect();
    format!("CREATE TABLE {} ({})", kind.table(), columns.join(", "))
}

/// A date's sort key, `None` for a text-only date: it has no place in the order.
fn date_key(date: &GenealogicalDate) -> Option<i64> {
    match date.modifier {
        GenealogicalDateBody::Structured(_) => Some(date.sort_value),
        GenealogicalDateBody::TextOnly { .. } => None,
    }
}

/// Inserts or replaces one row of `kind`'s list table, bound by [`row_values`].
pub(crate) fn upsert(dialect: Dialect, kind: ListedKind) -> String {
    let names: Vec<String> = table_columns(dialect, kind).into_iter().map(|(name, _)| name).collect();
    let mut binder = Binder::new(dialect);
    let mut placeholders = Vec::with_capacity(names.len());
    for _ in &names {
        placeholders.push(binder.bind(Value::Text(None)));
    }
    let updates: Vec<String> = names
        .iter()
        .skip(1)
        .map(|name| format!("{name} = excluded.{name}"))
        .collect();
    format!(
        "INSERT INTO {} ({}) VALUES ({}) ON CONFLICT (view_id) DO UPDATE SET {}",
        kind.table(),
        names.join(", "),
        placeholders.join(", "),
        updates.join(", ")
    )
}

/// The values [`upsert`] binds for `row`, in table-column order.
pub(crate) fn row_values(kind: ListedKind, row: &RowValues) -> Vec<Value> {
    let mut search = Vec::new();
    if let Some(human_id) = &row.human_id {
        search.push(fold(human_id));
    }
    let mut values = Vec::new();
    for (column, cell) in kind.columns().iter().zip(&row.cells) {
        match (column.ty, cell) {
            (ColumnType::Text, Some(ListCell::Text(text))) => {
                search.push(fold(text));
                values.push(Value::Text(Some(text.clone())));
                values.push(Value::Text(Some(fold(text))));
            }
            (ColumnType::Date, Some(ListCell::Date(date))) => {
                values.push(Value::Text(serde_json::to_string(date).ok()));
                values.push(Value::Int(date_key(date)));
            }
            (ColumnType::Number, Some(ListCell::Number(number))) => values.push(Value::Real(Some(*number))),
            (ColumnType::Choice, Some(ListCell::Choice(choice))) => values.push(Value::Text(Some(choice.clone()))),
            (ColumnType::Record, Some(ListCell::Record { id, title })) => {
                values.push(Value::Text(Some(id.clone())));
                values.push(Value::Text(title.clone()));
                values.push(Value::Text(title.as_deref().map(fold)));
            }
            (ColumnType::Tags, _) => {}
            (ColumnType::Text | ColumnType::Date | ColumnType::Choice | ColumnType::Record | ColumnType::Number, _) => {
                for (_, stored) in physical(column) {
                    values.push(stored.null());
                }
            }
        }
    }
    let mut row_values = vec![
        Value::Text(Some(row.id.clone())),
        Value::Text(row.human_id.clone()),
        Value::Text(Some(search.join("\n"))),
    ];
    row_values.extend(values);
    row_values
}

/// How one requested cell is read back from a result row: its type and its column aliases.
#[derive(Debug, Clone, Copy)]
pub(crate) enum CellRead {
    Text(usize),
    Date(usize),
    Number(usize),
    Choice(usize),
    Record(usize),
    Tags(usize),
}

impl CellRead {
    /// The alias of the cell's value; a record's title is under [`title_alias`].
    pub(crate) fn alias(index: usize) -> String {
        format!("c{index}")
    }

    /// The alias of a record cell's title.
    pub(crate) fn title_alias(index: usize) -> String {
        format!("c{index}_title")
    }
}

/// The SQL type a sort key is read back as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(crate) enum SortRead {
    Text,
    Int,
    Real,
}

/// A list query: its rows statement, its count statement, and how to read the rows.
#[derive(Debug)]
pub(crate) struct Select {
    pub(crate) rows: Statement,
    pub(crate) count: Statement,
    pub(crate) cells: Vec<CellRead>,
    pub(crate) sort: Option<SortRead>,
}

fn column_of(kind: ListedKind, id: &str) -> Result<ListColumn, DbError> {
    kind.column(id)
        .ok_or_else(|| DbError::Malformed(format!("the {kind:?} list has no column `{id}`")))
}

/// The expression a column sorts by, and how its key is read back.
fn sort_expression(dialect: Dialect, column: &ListColumn) -> Result<(String, SortRead), DbError> {
    let stem = stem(column);
    match column.ty {
        ColumnType::Text | ColumnType::Record => Ok((dialect.bytewise(&format!("t.{stem}_fold")), SortRead::Text)),
        ColumnType::Date => Ok((format!("t.{stem}_key"), SortRead::Int)),
        ColumnType::Number => Ok((format!("t.{stem}"), SortRead::Real)),
        ColumnType::Choice => Ok((dialect.bytewise(&format!("t.{stem}")), SortRead::Text)),
        ColumnType::Tags => Err(DbError::Malformed(format!(
            "the `{}` column cannot be sorted",
            column.id
        ))),
    }
}

/// The SQL test one filter makes.
fn predicate(binder: &mut Binder, kind: ListedKind, column: &str, test: &ListTest) -> Result<String, DbError> {
    let column = column_of(kind, column)?;
    let stem = stem(&column);
    let dialect = binder.dialect;
    let sql = match (column.ty, test) {
        (ColumnType::Text, ListTest::Contains(needle)) => {
            let needle = binder.text(&fold(needle));
            dialect.contains(&format!("t.{stem}_fold"), &needle)
        }
        (ColumnType::Text, ListTest::Is(value)) => format!("t.{stem}_fold = {}", binder.text(&fold(value))),
        (ColumnType::Text | ColumnType::Record, ListTest::IsEmpty) => format!("t.{stem} IS NULL"),
        (ColumnType::Date, ListTest::IsEmpty) => format!("t.{stem}_key IS NULL"),
        (ColumnType::Date, ListTest::Before(key)) => format!("t.{stem}_key < {}", binder.bind(Value::Int(Some(*key)))),
        (ColumnType::Date, ListTest::After(key)) => format!("t.{stem}_key > {}", binder.bind(Value::Int(Some(*key)))),
        (ColumnType::Date, ListTest::DatesBetween(from, to)) => {
            let from = binder.bind(Value::Int(Some(*from)));
            let to = binder.bind(Value::Int(Some(*to)));
            format!("t.{stem}_key BETWEEN {from} AND {to}")
        }
        (ColumnType::Number, ListTest::LessThan(value)) => {
            format!("t.{stem} < {}", binder.bind(Value::Real(Some(*value))))
        }
        (ColumnType::Number, ListTest::GreaterThan(value)) => {
            format!("t.{stem} > {}", binder.bind(Value::Real(Some(*value))))
        }
        (ColumnType::Number, ListTest::NumbersBetween(from, to)) => {
            let from = binder.bind(Value::Real(Some(*from)));
            let to = binder.bind(Value::Real(Some(*to)));
            format!("t.{stem} BETWEEN {from} AND {to}")
        }
        (ColumnType::Choice, ListTest::AnyOf(values)) if values.is_empty() => "1 = 0".to_owned(),
        (ColumnType::Choice, ListTest::AnyOf(values)) => {
            let folded: Vec<String> = values.iter().map(|value| fold(value)).collect();
            format!("t.{stem} IN ({})", binder.list(&folded))
        }
        (ColumnType::Choice | ColumnType::Tags, ListTest::NoneOf(values)) if values.is_empty() => "1 = 1".to_owned(),
        (ColumnType::Choice, ListTest::NoneOf(values)) => {
            let folded: Vec<String> = values.iter().map(|value| fold(value)).collect();
            format!("(t.{stem} IS NULL OR t.{stem} NOT IN ({}))", binder.list(&folded))
        }
        (ColumnType::Record, ListTest::Is(id)) => format!("t.{stem} = {}", binder.text(id)),
        (ColumnType::Tags, ListTest::AnyOf(tags)) if tags.is_empty() => "1 = 0".to_owned(),
        (ColumnType::Tags, ListTest::AnyOf(tags)) => format!("EXISTS ({})", tagged(binder, tags)),
        (ColumnType::Tags, ListTest::NoneOf(tags)) => format!("NOT EXISTS ({})", tagged(binder, tags)),
        (ColumnType::Tags, ListTest::AllOf(tags)) if tags.is_empty() => "1 = 1".to_owned(),
        (ColumnType::Tags, ListTest::AllOf(tags)) => {
            let mut all = Vec::with_capacity(tags.len());
            for tag in tags {
                all.push(format!("EXISTS ({})", tagged(binder, std::slice::from_ref(tag))));
            }
            all.join(" AND ")
        }
        (ty, test) => {
            return Err(DbError::Malformed(format!(
                "a {ty:?} column (`{}`) takes no {test:?} filter",
                column.id
            )));
        }
    };
    Ok(sql)
}

/// The `tagged` links from the row's record to one of `tags`.
fn tagged(binder: &mut Binder, tags: &[String]) -> String {
    format!(
        "SELECT 1 FROM record_links l WHERE l.relation = 'tagged' AND l.source = t.view_id AND l.target IN ({})",
        binder.list(tags)
    )
}

/// The conditions every matching row meets: listed (not merged), the search, the filters.
fn conditions(binder: &mut Binder, spec: &ListSpec) -> Result<Vec<String>, DbError> {
    let mut conditions = Vec::new();
    if let Some(merged) = spec.kind.merged_kind() {
        let kind = binder.text(merged.as_str());
        conditions.push(format!(
            "NOT EXISTS (SELECT 1 FROM identity_links m WHERE m.kind = {kind} AND m.member = t.view_id)"
        ));
    }
    if let Some(search) = spec.search.as_deref().filter(|search| !search.trim().is_empty()) {
        let needle = binder.text(&fold(search.trim()));
        conditions.push(binder.dialect.contains("t.search", &needle));
    }
    for filter in &spec.filters {
        conditions.push(predicate(binder, spec.kind, &filter.column, &filter.test)?);
    }
    Ok(conditions)
}

fn where_clause(conditions: &[String]) -> String {
    if conditions.is_empty() {
        String::new()
    } else {
        format!("WHERE {}", conditions.join(" AND "))
    }
}

/// The rows after a held one, in the query's order: those whose sort value comes after its value, then
/// on a tie those whose `human_id` and id do — with rows lacking a value after every row that has one.
/// SQLite's `?` binds by position, so each value is bound where the text names it.
fn keyset(
    binder: &mut Binder,
    sort: Option<(&str, SortDirection)>,
    cursor: &super::ListCursor,
    tie: (&str, &str),
) -> String {
    let Some((expression, direction)) = sort else {
        return tie_after(binder, cursor, tie);
    };
    let Some(key) = &cursor.sort else {
        let tie_after = tie_after(binder, cursor, tie);
        return format!("({expression} IS NULL AND {tie_after})");
    };
    let comparison = match direction {
        SortDirection::Ascending => ">",
        SortDirection::Descending => "<",
    };
    let key = match key {
        SortKey::Text(text) => Value::Text(Some(text.clone())),
        SortKey::Int(int) => Value::Int(Some(*int)),
        SortKey::Real(real) => Value::Real(Some(*real)),
    };
    let past = binder.bind(key.clone());
    let tied = binder.bind(key);
    let tie_after = tie_after(binder, cursor, tie);
    format!("({expression} {comparison} {past} OR {expression} IS NULL OR ({expression} = {tied} AND {tie_after}))")
}

/// The rows after the held one by `human_id`, then id.
fn tie_after(binder: &mut Binder, cursor: &super::ListCursor, tie: (&str, &str)) -> String {
    let (human_id, view_id) = tie;
    let held = cursor.human_id.as_deref().unwrap_or_default();
    let after = binder.text(held);
    let same = binder.text(held);
    let id = binder.text(&cursor.id);
    format!("({human_id} > {after} OR ({human_id} = {same} AND {view_id} > {id}))")
}

/// The list query for `spec`.
pub(crate) fn select(dialect: Dialect, spec: &ListSpec) -> Result<Select, DbError> {
    let kind = spec.kind;
    let human_id = dialect.bytewise("COALESCE(t.human_id, '')");
    let view_id = dialect.bytewise("t.view_id");

    let sort = match &spec.sort {
        Some(sort) => {
            let column = column_of(kind, &sort.column)?;
            let (expression, read) = sort_expression(dialect, &column)?;
            Some((expression, read, sort.direction))
        }
        None => None,
    };

    let mut fields = vec!["t.view_id".to_owned(), "t.human_id".to_owned()];
    if let Some((expression, _, _)) = &sort {
        fields.push(format!("{expression} AS sort_key"));
    }
    let mut cells = Vec::with_capacity(spec.columns.len());
    for (index, id) in spec.columns.iter().enumerate() {
        let column = column_of(kind, id)?;
        let stem = stem(&column);
        let alias = CellRead::alias(index);
        let (field, read) = match column.ty {
            ColumnType::Text => (format!("t.{stem} AS {alias}"), CellRead::Text(index)),
            ColumnType::Date => (format!("t.{stem} AS {alias}"), CellRead::Date(index)),
            ColumnType::Number => (format!("t.{stem} AS {alias}"), CellRead::Number(index)),
            ColumnType::Choice => (format!("t.{stem} AS {alias}"), CellRead::Choice(index)),
            ColumnType::Record => (
                format!(
                    "t.{stem} AS {alias}, t.{stem}_title AS {}",
                    CellRead::title_alias(index)
                ),
                CellRead::Record(index),
            ),
            ColumnType::Tags => (format!("{} AS {alias}", dialect.tag_list()), CellRead::Tags(index)),
        };
        fields.push(field);
        cells.push(read);
    }

    let mut count = Binder::new(dialect);
    let count_conditions = conditions(&mut count, spec)?;
    let count = count.finish(format!(
        "SELECT COUNT(*) AS total FROM {} t {}",
        kind.table(),
        where_clause(&count_conditions)
    ));

    let mut rows = Binder::new(dialect);
    let mut row_conditions = conditions(&mut rows, spec)?;
    let (offset, limit) = match &spec.window {
        ListWindow::Offset { offset, limit } => (*offset, *limit),
        ListWindow::After { cursor, limit } => {
            let sorted = sort
                .as_ref()
                .map(|(expression, _, direction)| (expression.as_str(), *direction));
            row_conditions.push(keyset(&mut rows, sorted, cursor, (&human_id, &view_id)));
            (0, *limit)
        }
    };
    let mut order = Vec::new();
    if let Some((expression, _, direction)) = &sort {
        let direction = match direction {
            SortDirection::Ascending => "ASC",
            SortDirection::Descending => "DESC",
        };
        order.push(format!("{expression} {direction} NULLS LAST"));
    }
    order.push(format!("{human_id} ASC"));
    order.push(format!("{view_id} ASC"));
    let limit = match dialect {
        // A negative LIMIT is SQLite's "no limit"; Postgres takes NULL as `LIMIT ALL`.
        Dialect::Sqlite => rows.bind(Value::Int(Some(limit.map_or(-1, i64::from)))),
        Dialect::Postgres => rows.bind(Value::Int(limit.map(i64::from))),
    };
    let offset = rows.bind(Value::Int(Some(i64::from(offset))));
    let sql = format!(
        "SELECT {} FROM {} t {} ORDER BY {} LIMIT {limit} OFFSET {offset}",
        fields.join(", "),
        kind.table(),
        where_clause(&row_conditions),
        order.join(", ")
    );
    Ok(Select {
        rows: rows.finish(sql),
        count,
        cells,
        sort: sort.map(|(_, read, _)| read),
    })
}

/// Which tag uses a statement reads: one tag's records, or every tag's count.
#[derive(Debug, Clone, Copy)]
pub(crate) enum TagUses<'a> {
    Of(&'a str),
    Counted,
}

/// The records carrying tags through the `tagged` links, each read as its cluster's root (ADR 0039
/// §5) so a cluster whose records share a tag is one use: `TagUses::Of` reads one tag's as `(k, record)` in
/// group order — `k` the kind's index in [`ListedKind::ALL`] — then `human_id`;
/// `TagUses::Counted` reads `(tag, uses)` for every tag in use.
pub(crate) fn tag_uses(dialect: Dialect, uses: TagUses<'_>) -> Statement {
    let mut binder = Binder::new(dialect);
    let mut branches = Vec::new();
    for (index, kind) in ListedKind::ALL.iter().enumerate() {
        if kind.columns().iter().all(|column| column.ty != ColumnType::Tags) {
            continue;
        }
        let (merge, record) = match kind.merged_kind() {
            Some(merged) => (
                format!(
                    "LEFT JOIN identity_links m ON m.kind = {} AND m.member = l.source",
                    binder.text(merged.as_str())
                ),
                "COALESCE(m.root, l.source)",
            ),
            None => (String::new(), "l.source"),
        };
        let of = match uses {
            TagUses::Of(tag) => format!(" AND l.target = {}", binder.text(tag)),
            TagUses::Counted => String::new(),
        };
        branches.push(format!(
            "SELECT CAST({index} AS BIGINT) AS k, {record} AS record, r.human_id AS human_id, l.target AS tag \
             FROM record_links l {merge} JOIN {table} r ON r.view_id = {record} \
             WHERE l.relation = 'tagged'{of}",
            table = kind.table()
        ));
    }
    let union = branches.join(" UNION ALL ");
    let sql = match uses {
        TagUses::Of(_) => format!(
            "SELECT k, record FROM (SELECT DISTINCT k, record, human_id FROM ({union}) u) d ORDER BY k, {}, record",
            dialect.bytewise("COALESCE(d.human_id, '')")
        ),
        TagUses::Counted => format!(
            "SELECT tag, COUNT(*) AS uses FROM (SELECT DISTINCT k, record, tag FROM ({union}) u) d GROUP BY tag"
        ),
    };
    binder.finish(sql)
}

#[cfg(test)]
mod tests {
    use super::{Dialect, create_table, upsert};
    use crate::list_table::ListedKind;

    #[test]
    fn every_kind_has_a_table_and_an_upsert_on_both_engines() {
        for kind in ListedKind::ALL {
            for dialect in [Dialect::Sqlite, Dialect::Postgres] {
                assert!(create_table(dialect, *kind).starts_with("CREATE TABLE"));
                assert!(upsert(dialect, *kind).contains("ON CONFLICT (view_id)"));
            }
        }
    }
}
