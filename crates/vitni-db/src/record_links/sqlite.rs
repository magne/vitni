//! The SQLite half of the record links index (ADR 0047) — see the [module header](super) for what the
//! table holds and why the index exists.

use std::marker::PhantomData;

use async_trait::async_trait;
use cqrs_es::{EventEnvelope, Query};
use sqlx::{Pool, Row, Sqlite};
use vitni_core::citation::CitationView;
use vitni_core::event::EventView;
use vitni_core::family::FamilyView;
use vitni_core::person::PersonView;
use vitni_core::place::PlaceView;
use vitni_core::source::SourceView;

use super::{LinkingRecord, RECORD_LINKS_TABLE, RecordLink};
use crate::sqlite_query;
use crate::store::DbError;

const CREATE_RECORD_LINKS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS record_links (
    relation  TEXT NOT NULL,
    source    TEXT NOT NULL,
    target    TEXT NOT NULL,
    PRIMARY KEY (relation, source, target)
)";

const CREATE_RECORD_LINKS_TARGET_INDEX: &str =
    "CREATE INDEX IF NOT EXISTS record_links_by_target ON record_links (relation, target)";

/// Wraps a `sqlx` error with what the index was doing.
fn backend(action: &'static str) -> impl Fn(sqlx::Error) -> DbError {
    move |error| DbError::Backend(format!("{action}: {error}"))
}

/// Creates the record links table filled from the projections, unless it exists — so a workspace whose
/// projections predate it gets it filled. The table is created and filled in one transaction: an open
/// cut short leaves no table, never an empty one the next open would take for filled.
///
/// # Errors
///
/// A [`DbError`] if reading a projection or writing the index fails.
pub(crate) async fn create_filled(pool: &Pool<Sqlite>) -> Result<(), DbError> {
    let existed = sqlx::query("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?")
        .bind(RECORD_LINKS_TABLE)
        .fetch_optional(pool)
        .await
        .map_err(backend("looking for the record links table"))?
        .is_some();
    if existed {
        return Ok(());
    }
    let mut links = Vec::new();
    links.extend(links_of::<PersonView>(pool).await?);
    links.extend(links_of::<FamilyView>(pool).await?);
    links.extend(links_of::<EventView>(pool).await?);
    links.extend(links_of::<PlaceView>(pool).await?);
    links.extend(links_of::<SourceView>(pool).await?);
    links.extend(links_of::<CitationView>(pool).await?);
    let mut tx = pool
        .begin()
        .await
        .map_err(backend("opening a record links transaction"))?;
    for statement in [CREATE_RECORD_LINKS_TABLE, CREATE_RECORD_LINKS_TARGET_INDEX] {
        sqlx::query(statement)
            .execute(&mut *tx)
            .await
            .map_err(backend("creating the record links table"))?;
    }
    for (relation, source, target) in &links {
        insert_link(&mut tx, *relation, source, target).await?;
    }
    tx.commit().await.map_err(backend("committing the record links"))
}

/// Every `(relation, source, target)` the projections of one kind hold.
async fn links_of<V: LinkingRecord>(pool: &Pool<Sqlite>) -> Result<Vec<(RecordLink, String, String)>, DbError> {
    let views: Vec<V> = sqlite_query::list_views(pool, V::VIEW_TABLE).await?;
    let mut links = Vec::new();
    for view in &views {
        let Some(source) = view.linking_id() else { continue };
        for (relation, target) in view.links() {
            links.push((relation, source.clone(), target));
        }
    }
    Ok(links)
}

/// A `cqrs-es` query that keeps one kind's references in step with its projection. Must be appended
/// *after* that aggregate's `GenericQuery`, so the projection it reads is already up to date.
pub(crate) struct RecordLinksQuery<V> {
    pool: Pool<Sqlite>,
    view: PhantomData<fn() -> V>,
}

impl<V> RecordLinksQuery<V> {
    /// Wraps the pool the projection and index tables share.
    pub(crate) fn new(pool: Pool<Sqlite>) -> Self {
        Self {
            pool,
            view: PhantomData,
        }
    }
}

#[async_trait]
impl<V: LinkingRecord> Query<V::State> for RecordLinksQuery<V> {
    async fn dispatch(&self, aggregate_id: &str, events: &[EventEnvelope<V::State>]) {
        if !events.iter().any(|envelope| V::changes_links(&envelope.payload)) {
            return;
        }
        if let Err(error) = reindex_source::<V>(&self.pool, aggregate_id).await {
            crate::projection_failures::report(RECORD_LINKS_TABLE, &error);
        }
    }
}

/// Mirrors one record's references from its projection.
async fn reindex_source<V: LinkingRecord>(pool: &Pool<Sqlite>, source: &str) -> Result<(), DbError> {
    let view = sqlite_query::find_view_by_id::<V>(pool, V::VIEW_TABLE, source).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(backend("opening a record links transaction"))?;
    for relation in V::RELATIONS {
        sqlx::query(&format!(
            "DELETE FROM {RECORD_LINKS_TABLE} WHERE relation = ? AND source = ?"
        ))
        .bind(relation.as_str())
        .bind(source)
        .execute(&mut *tx)
        .await
        .map_err(backend("clearing a record's links"))?;
    }
    if let Some(view) = view {
        insert_links(&mut tx, source, &view).await?;
    }
    tx.commit().await.map_err(backend("committing the record links"))
}

/// Inserts one row per reference `view` holds.
async fn insert_links<V: LinkingRecord>(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    source: &str,
    view: &V,
) -> Result<(), DbError> {
    for (relation, target) in view.links() {
        insert_link(tx, relation, source, &target).await?;
    }
    Ok(())
}

/// Inserts one reference, unless it is there already.
async fn insert_link(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    relation: RecordLink,
    source: &str,
    target: &str,
) -> Result<(), DbError> {
    sqlx::query(&format!(
        "INSERT OR IGNORE INTO {RECORD_LINKS_TABLE} (relation, source, target) VALUES (?, ?, ?)"
    ))
    .bind(relation.as_str())
    .bind(source)
    .bind(target)
    .execute(&mut **tx)
    .await
    .map_err(backend("inserting a record link"))?;
    Ok(())
}

/// Rebuilds the whole index from the (already-rebuilt) projections — the maintenance path
/// `Store::rebuild_projections` drives (ADR 0010), through the same inserts as the live path.
///
/// # Errors
///
/// A [`DbError`] if reading a projection or writing the index fails.
pub(crate) async fn rebuild_index(pool: &Pool<Sqlite>) -> Result<(), DbError> {
    rebuild_kind::<PersonView>(pool).await?;
    rebuild_kind::<FamilyView>(pool).await?;
    rebuild_kind::<EventView>(pool).await?;
    rebuild_kind::<PlaceView>(pool).await?;
    rebuild_kind::<SourceView>(pool).await?;
    rebuild_kind::<CitationView>(pool).await
}

/// Rebuilds one kind's references from its projection.
async fn rebuild_kind<V: LinkingRecord>(pool: &Pool<Sqlite>) -> Result<(), DbError> {
    let views: Vec<V> = sqlite_query::list_views(pool, V::VIEW_TABLE).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(backend("opening a record links transaction"))?;
    for relation in V::RELATIONS {
        sqlx::query(&format!("DELETE FROM {RECORD_LINKS_TABLE} WHERE relation = ?"))
            .bind(relation.as_str())
            .execute(&mut *tx)
            .await
            .map_err(backend("clearing the record links"))?;
    }
    for view in &views {
        let Some(source) = view.linking_id() else { continue };
        insert_links(&mut tx, &source, view).await?;
    }
    tx.commit().await.map_err(backend("committing the record links"))
}

/// Every `(source, target)` of `relation` whose target is one of `targets`, in that order.
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn linking(
    pool: &Pool<Sqlite>,
    relation: RecordLink,
    targets: &[String],
) -> Result<Vec<(String, String)>, DbError> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let targets = serde_json::to_string(targets).map_err(|e| DbError::Backend(e.to_string()))?;
    let rows = sqlx::query(&format!(
        "SELECT source, target FROM {RECORD_LINKS_TABLE} \
         WHERE relation = ? AND target IN (SELECT value FROM json_each(?)) ORDER BY source, target"
    ))
    .bind(relation.as_str())
    .bind(targets)
    .fetch_all(pool)
    .await
    .map_err(backend("reading the record links"))?;
    Ok(rows.iter().map(|row| (row.get("source"), row.get("target"))).collect())
}
