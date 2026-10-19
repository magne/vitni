//! The Postgres half of the list tables (ADR 0052 §4–5) — see the [module header](super) for what the
//! tables hold and why.

use std::marker::PhantomData;

use async_trait::async_trait;
use cqrs_es::{EventEnvelope, Query};
use sqlx::postgres::{PgArguments, PgRow};
use sqlx::{Pool, Postgres, Row};
use vitni_core::citation::CitationView;
use vitni_core::dna_match::DnaMatchView;
use vitni_core::dna_test::DnaTestView;
use vitni_core::event::EventView;
use vitni_core::family::FamilyView;
use vitni_core::media::MediaView;
use vitni_core::note::NoteView;
use vitni_core::person::PersonView;
use vitni_core::place::PlaceView;
use vitni_core::repository::RepositoryView;
use vitni_core::research_note::ResearchNoteView;
use vitni_core::source::SourceView;
use vitni_core::tag::TagView;

use super::rows::{DEPENDENTS, Refs, RowValues, Wanted};
use super::sql::{self, CellRead, Dialect, SortRead, Value};
use super::{ListCell, ListCursor, ListResult, ListRow, ListSpec, ListedKind, ListedRecord, SortKey};
use crate::postgres_query;
use crate::store::DbError;

const DIALECT: Dialect = Dialect::Postgres;

/// Views computed into rows per batch, bounding what a fill holds at once.
const BATCH: usize = 2_000;

/// Wraps a `sqlx` error with what the list tables were doing.
fn backend(action: &'static str) -> impl Fn(sqlx::Error) -> DbError {
    move |error| DbError::Backend(format!("{action}: {error}"))
}

type Statement<'q> = sqlx::query::Query<'q, Postgres, PgArguments>;

fn bind_all<'q>(mut query: Statement<'q>, values: &[Value]) -> Statement<'q> {
    for value in values {
        query = match value {
            Value::Text(text) => query.bind(text.clone()),
            Value::Int(int) => query.bind(*int),
            Value::Real(real) => query.bind(*real),
        };
    }
    query
}

/// Creates and fills every list table that is missing or shaped differently from its catalogue, so a
/// workspace whose projections predate a table gets it. A workspace without them also predates the
/// `tagged` links (ADR 0052 §6), so the record links are rebuilt first. Each table is created and filled
/// in one transaction: an open cut short leaves no table, never an empty one taken for filled.
///
/// # Errors
///
/// A [`DbError`] if reading a projection or writing a table fails.
pub(crate) async fn create_filled(pool: &Pool<Postgres>) -> Result<(), DbError> {
    let mut stale = Vec::new();
    for kind in ListedKind::ALL {
        let rows = sqlx::query(
            "SELECT column_name::text AS name FROM information_schema.columns \
             WHERE table_schema = current_schema() AND table_name = $1 ORDER BY ordinal_position",
        )
        .bind(kind.table())
        .fetch_all(pool)
        .await
        .map_err(backend("reading a list table's columns"))?;
        let found: Vec<String> = rows.iter().map(|row| row.get("name")).collect();
        let expected: Vec<String> = sql::table_columns(DIALECT, *kind)
            .into_iter()
            .map(|(name, _)| name)
            .collect();
        if found != expected {
            stale.push(*kind);
        }
    }
    if !stale.is_empty() {
        crate::record_links::postgres::rebuild_index(pool).await?;
        for kind in stale {
            refill(pool, kind, true).await?;
        }
    }
    for kind in ListedKind::ALL {
        let mut indexes = vec![sql::order_index(DIALECT, *kind)];
        indexes.extend(sql::sort_indexes(DIALECT, *kind));
        for index in indexes {
            sqlx::query(&index)
                .execute(pool)
                .await
                .map_err(backend("indexing a list table"))?;
        }
    }
    Ok(())
}

/// Refills every list table from the (already-rebuilt) projections — the maintenance path
/// `Store::rebuild_projections` drives (ADR 0010).
///
/// # Errors
///
/// A [`DbError`] if reading a projection or writing a table fails.
pub(crate) async fn rebuild(pool: &Pool<Postgres>) -> Result<(), DbError> {
    for kind in ListedKind::ALL {
        refill(pool, *kind, false).await?;
    }
    Ok(())
}

/// Every row of `kind`, computed from its projections, written in one transaction — the table dropped
/// and recreated first when `recreate`, else cleared.
async fn refill(pool: &Pool<Postgres>, kind: ListedKind, recreate: bool) -> Result<(), DbError> {
    let rows = match kind {
        ListedKind::Person => all_rows::<PersonView>(pool).await?,
        ListedKind::Family => all_rows::<FamilyView>(pool).await?,
        ListedKind::Event => all_rows::<EventView>(pool).await?,
        ListedKind::Place => all_rows::<PlaceView>(pool).await?,
        ListedKind::Source => all_rows::<SourceView>(pool).await?,
        ListedKind::Citation => all_rows::<CitationView>(pool).await?,
        ListedKind::Repository => all_rows::<RepositoryView>(pool).await?,
        ListedKind::Media => all_rows::<MediaView>(pool).await?,
        ListedKind::Note => all_rows::<NoteView>(pool).await?,
        ListedKind::ResearchNote => all_rows::<ResearchNoteView>(pool).await?,
        ListedKind::DnaTest => all_rows::<DnaTestView>(pool).await?,
        ListedKind::DnaMatch => all_rows::<DnaMatchView>(pool).await?,
        ListedKind::Tag => all_rows::<TagView>(pool).await?,
    };
    let mut tx = pool
        .begin()
        .await
        .map_err(backend("opening a list table transaction"))?;
    let resets = if recreate {
        vec![
            format!("DROP TABLE IF EXISTS {}", kind.table()),
            sql::create_table(DIALECT, kind),
        ]
    } else {
        vec![format!("DELETE FROM {}", kind.table())]
    };
    for statement in resets {
        sqlx::query(&statement)
            .execute(&mut *tx)
            .await
            .map_err(backend("resetting a list table"))?;
    }
    let upsert = sql::upsert(DIALECT, kind);
    for row in &rows {
        bind_all(sqlx::query(&upsert), &sql::row_values(kind, row))
            .execute(&mut *tx)
            .await
            .map_err(backend("writing a list row"))?;
    }
    tx.commit().await.map_err(backend("committing a list table"))
}

/// Every record's row of one kind, computed batch by batch.
async fn all_rows<V: ListedRecord>(pool: &Pool<Postgres>) -> Result<Vec<RowValues>, DbError> {
    let views: Vec<V> = postgres_query::list_views(pool, V::VIEW_TABLE).await?;
    let mut rows = Vec::with_capacity(views.len());
    for batch in views.chunks(BATCH) {
        rows.extend(compute(pool, batch).await?);
    }
    Ok(rows)
}

/// The rows of `views`, reading the records they reference.
async fn compute<V: ListedRecord>(pool: &Pool<Postgres>, views: &[V]) -> Result<Vec<RowValues>, DbError> {
    let mut wanted = Wanted::default();
    for view in views {
        view.wants(&mut wanted);
    }
    let refs = load_refs(pool, &wanted).await?;
    Ok(views.iter().filter_map(|view| view.row(&refs)).collect())
}

/// The records `wanted` names, keyed by id.
async fn load_refs(pool: &Pool<Postgres>, wanted: &Wanted) -> Result<Refs, DbError> {
    Ok(Refs {
        persons: by_id::<PersonView>(pool, &wanted.persons).await?,
        events: by_id::<EventView>(pool, &wanted.events).await?,
        places: by_id::<PlaceView>(pool, &wanted.places).await?,
        sources: by_id::<SourceView>(pool, &wanted.sources).await?,
        dna_tests: by_id::<DnaTestView>(pool, &wanted.dna_tests).await?,
    })
}

async fn by_id<V: ListedRecord>(
    pool: &Pool<Postgres>,
    ids: &std::collections::BTreeSet<String>,
) -> Result<std::collections::HashMap<String, V>, DbError> {
    let ids: Vec<String> = ids.iter().cloned().collect();
    let views: Vec<V> = postgres_query::views_by_ids(pool, V::VIEW_TABLE, &ids).await?;
    Ok(views
        .into_iter()
        .filter_map(|view| view.list_id().map(|id| (id, view)))
        .collect())
}

/// Recomputes the rows of `ids`, deleting the row of an id with no projection.
async fn refresh<V: ListedRecord>(pool: &Pool<Postgres>, ids: &[String]) -> Result<(), DbError> {
    if ids.is_empty() {
        return Ok(());
    }
    let views: Vec<V> = postgres_query::views_by_ids(pool, V::VIEW_TABLE, ids).await?;
    let rows = compute(pool, &views).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(backend("opening a list table transaction"))?;
    let upsert = sql::upsert(DIALECT, V::KIND);
    for row in &rows {
        bind_all(sqlx::query(&upsert), &sql::row_values(V::KIND, row))
            .execute(&mut *tx)
            .await
            .map_err(backend("writing a list row"))?;
    }
    for id in ids {
        if rows.iter().all(|row| &row.id != id) {
            sqlx::query(&format!("DELETE FROM {} WHERE view_id = $1", V::KIND.table()))
                .bind(id)
                .execute(&mut *tx)
                .await
                .map_err(backend("deleting a list row"))?;
        }
    }
    tx.commit().await.map_err(backend("committing list rows"))
}

/// Recomputes the rows of `ids`, of `kind`.
async fn refresh_kind(pool: &Pool<Postgres>, kind: ListedKind, ids: &[String]) -> Result<(), DbError> {
    match kind {
        ListedKind::Person => refresh::<PersonView>(pool, ids).await,
        ListedKind::Family => refresh::<FamilyView>(pool, ids).await,
        ListedKind::Event => refresh::<EventView>(pool, ids).await,
        ListedKind::Place => refresh::<PlaceView>(pool, ids).await,
        ListedKind::Source => refresh::<SourceView>(pool, ids).await,
        ListedKind::Citation => refresh::<CitationView>(pool, ids).await,
        ListedKind::Repository => refresh::<RepositoryView>(pool, ids).await,
        ListedKind::Media => refresh::<MediaView>(pool, ids).await,
        ListedKind::Note => refresh::<NoteView>(pool, ids).await,
        ListedKind::ResearchNote => refresh::<ResearchNoteView>(pool, ids).await,
        ListedKind::DnaTest => refresh::<DnaTestView>(pool, ids).await,
        ListedKind::DnaMatch => refresh::<DnaMatchView>(pool, ids).await,
        ListedKind::Tag => refresh::<TagView>(pool, ids).await,
    }
}

/// A `cqrs-es` query that keeps one kind's list rows in step with its projection, and the rows of the
/// records that read it. Must be appended *after* that aggregate's projection and record links queries,
/// so the projection and the links it reads are already up to date.
pub(crate) struct ListTableQuery<V> {
    pool: Pool<Postgres>,
    view: PhantomData<fn() -> V>,
}

impl<V> ListTableQuery<V> {
    /// Wraps the pool the projection and list tables share.
    pub(crate) fn new(pool: Pool<Postgres>) -> Self {
        Self {
            pool,
            view: PhantomData,
        }
    }
}

#[async_trait]
impl<V: ListedRecord> Query<V::State> for ListTableQuery<V> {
    async fn dispatch(&self, aggregate_id: &str, _events: &[EventEnvelope<V::State>]) {
        if let Err(error) = relist::<V>(&self.pool, aggregate_id).await {
            crate::projection_failures::report(V::KIND.table(), &error);
        }
    }
}

/// Recomputes the record's row, then the rows of the records whose rows read it.
async fn relist<V: ListedRecord>(pool: &Pool<Postgres>, id: &str) -> Result<(), DbError> {
    let id = [id.to_owned()];
    refresh::<V>(pool, &id).await?;
    for (dependent, relation, target) in DEPENDENTS {
        if *target != V::KIND {
            continue;
        }
        let sources: Vec<String> = crate::record_links::postgres::linking(pool, *relation, &id)
            .await?
            .into_iter()
            .map(|(source, _)| source)
            .collect();
        refresh_kind(pool, *dependent, &sources).await?;
    }
    Ok(())
}

/// Evaluates `spec` over its kind's list table.
///
/// # Errors
///
/// [`DbError::Malformed`] for a column or test the kind's catalogue does not take, or a [`DbError`] if
/// the query fails.
pub(crate) async fn query(pool: &Pool<Postgres>, spec: &ListSpec) -> Result<ListResult, DbError> {
    let select = sql::select(DIALECT, spec)?;
    let total = match &select.count {
        Some(count) => {
            let total: i64 = bind_all(sqlx::query(&count.sql), &count.binds)
                .fetch_one(pool)
                .await
                .map_err(backend("counting a list"))?
                .get("total");
            Some(u64::try_from(total).unwrap_or_default())
        }
        None => None,
    };
    let found = bind_all(sqlx::query(&select.rows.sql), &select.rows.binds)
        .fetch_all(pool)
        .await
        .map_err(backend("reading a list"))?;
    let mut rows = Vec::with_capacity(found.len());
    for row in &found {
        rows.push(read_row(row, &select.cells, select.sort)?);
    }
    Ok(ListResult { rows, total })
}

/// The records carrying `tag`, each as its cluster's root, by kind in [`ListedKind::ALL`] order, then
/// `human_id`.
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn tag_uses(pool: &Pool<Postgres>, tag: &str) -> Result<Vec<(ListedKind, String)>, DbError> {
    let statement = sql::tag_uses(DIALECT, sql::TagUses::Of(tag));
    let rows = bind_all(sqlx::query(&statement.sql), &statement.binds)
        .fetch_all(pool)
        .await
        .map_err(backend("reading a tag's uses"))?;
    let mut uses = Vec::with_capacity(rows.len());
    for row in &rows {
        let index: i64 = row.get("k");
        let kind = usize::try_from(index)
            .ok()
            .and_then(|index| ListedKind::ALL.get(index))
            .ok_or_else(|| DbError::Backend(format!("a tag use of no listed kind ({index})")))?;
        uses.push((*kind, row.get("record")));
    }
    Ok(uses)
}

/// How many records carry each tag in use, a cluster counted once.
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn tag_use_counts(pool: &Pool<Postgres>) -> Result<Vec<(String, u64)>, DbError> {
    let statement = sql::tag_uses(DIALECT, sql::TagUses::Counted);
    let rows = bind_all(sqlx::query(&statement.sql), &statement.binds)
        .fetch_all(pool)
        .await
        .map_err(backend("counting tag uses"))?;
    Ok(rows
        .iter()
        .map(|row| {
            let uses: i64 = row.get("uses");
            (row.get("tag"), u64::try_from(uses).unwrap_or_default())
        })
        .collect())
}

fn read_row(row: &PgRow, cells: &[CellRead], sort: Option<SortRead>) -> Result<ListRow, DbError> {
    let id: String = row.get("view_id");
    let human_id: Option<String> = row.get("human_id");
    let sort = match sort {
        None => None,
        Some(SortRead::Text) => row.get::<Option<String>, _>("sort_key").map(SortKey::Text),
        Some(SortRead::Int) => row.get::<Option<i64>, _>("sort_key").map(SortKey::Int),
        Some(SortRead::Real) => row.get::<Option<f64>, _>("sort_key").map(SortKey::Real),
    };
    let mut values = Vec::with_capacity(cells.len());
    for cell in cells {
        values.push(read_cell(row, *cell)?);
    }
    Ok(ListRow {
        cursor: ListCursor {
            sort,
            human_id: human_id.clone(),
            id: id.clone(),
        },
        id,
        human_id,
        cells: values,
    })
}

fn read_cell(row: &PgRow, cell: CellRead) -> Result<Option<ListCell>, DbError> {
    let text = |index| row.get::<Option<String>, _>(CellRead::alias(index).as_str());
    Ok(match cell {
        CellRead::Text(index) => text(index).map(ListCell::Text),
        CellRead::Choice(index) => text(index).map(ListCell::Choice),
        CellRead::Number(index) => row
            .get::<Option<f64>, _>(CellRead::alias(index).as_str())
            .map(ListCell::Number),
        CellRead::Date(index) => match text(index) {
            Some(json) => Some(ListCell::Date(
                serde_json::from_str(&json).map_err(|e| DbError::Backend(format!("decoding a list date: {e}")))?,
            )),
            None => None,
        },
        CellRead::Record(index) => text(index).map(|id| ListCell::Record {
            id,
            title: row.get(CellRead::title_alias(index).as_str()),
        }),
        CellRead::Tags(index) => Some(ListCell::Tags(super::tag_ids(text(index).as_deref())?)),
    })
}
