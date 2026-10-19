//! The Postgres half of the record links index (ADR 0047) — see the [module header](super) for what the
//! table holds and why the index exists.

use std::collections::BTreeSet;
use std::marker::PhantomData;

use async_trait::async_trait;
use cqrs_es::{EventEnvelope, Query};
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

use super::{LinkingRecord, RECORD_LINKS_TABLE, RecordLink};
use crate::postgres_query;
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
pub(crate) async fn create_filled(pool: &Pool<Postgres>) -> Result<(), DbError> {
    let existed: bool = sqlx::query("SELECT to_regclass($1) IS NOT NULL AS existed")
        .bind(RECORD_LINKS_TABLE)
        .fetch_one(pool)
        .await
        .map_err(backend("looking for the record links table"))?
        .get("existed");
    if existed {
        return Ok(());
    }
    let links = all_links(pool).await?;
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

/// Every `(relation, source, target)` the projections of every linking kind hold.
async fn all_links(pool: &Pool<Postgres>) -> Result<Vec<(RecordLink, String, String)>, DbError> {
    let mut links = Vec::new();
    links.extend(links_of::<PersonView>(pool).await?);
    links.extend(links_of::<FamilyView>(pool).await?);
    links.extend(links_of::<EventView>(pool).await?);
    links.extend(links_of::<PlaceView>(pool).await?);
    links.extend(links_of::<SourceView>(pool).await?);
    links.extend(links_of::<CitationView>(pool).await?);
    links.extend(links_of::<RepositoryView>(pool).await?);
    links.extend(links_of::<NoteView>(pool).await?);
    links.extend(links_of::<MediaView>(pool).await?);
    links.extend(links_of::<ResearchNoteView>(pool).await?);
    links.extend(links_of::<DnaTestView>(pool).await?);
    links.extend(links_of::<DnaMatchView>(pool).await?);
    Ok(links)
}

/// Every `(relation, source, target)` the projections of one kind hold.
async fn links_of<V: LinkingRecord>(pool: &Pool<Postgres>) -> Result<Vec<(RecordLink, String, String)>, DbError> {
    let views: Vec<V> = postgres_query::list_views(pool, V::VIEW_TABLE).await?;
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
/// *after* that aggregate's projection query, so the projection it reads is already up to date.
pub(crate) struct RecordLinksQuery<V> {
    pool: Pool<Postgres>,
    view: PhantomData<fn() -> V>,
}

impl<V> RecordLinksQuery<V> {
    /// Wraps the pool the projection and index tables share.
    pub(crate) fn new(pool: Pool<Postgres>) -> Self {
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

/// Mirrors one record's references from its projection, marking each target it no longer references
/// dirty for its match keys and pairs.
async fn reindex_source<V: LinkingRecord>(pool: &Pool<Postgres>, source: &str) -> Result<(), DbError> {
    let view = postgres_query::find_view_by_id::<V>(pool, V::VIEW_TABLE, source).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(backend("opening a record links transaction"))?;
    let mut dropped = BTreeSet::new();
    for relation in V::RELATIONS {
        let rows = sqlx::query(&format!(
            "DELETE FROM {RECORD_LINKS_TABLE} WHERE relation = $1 AND source = $2 RETURNING target"
        ))
        .bind(relation.as_str())
        .bind(source)
        .fetch_all(&mut *tx)
        .await
        .map_err(backend("clearing a record's links"))?;
        for row in rows {
            dropped.insert((*relation, row.get::<String, _>("target")));
        }
    }
    if let Some(view) = view {
        for link in view.links() {
            dropped.remove(&link);
        }
        insert_links(&mut tx, source, &view).await?;
    }
    for (relation, target) in &dropped {
        let Some(kind) = relation.rematched_kind() else {
            continue;
        };
        crate::match_keys::postgres::mark_dirty(&mut tx, kind, target).await?;
    }
    tx.commit().await.map_err(backend("committing the record links"))
}

/// Inserts one row per reference `view` holds.
async fn insert_links<V: LinkingRecord>(
    tx: &mut sqlx::Transaction<'_, Postgres>,
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
    tx: &mut sqlx::Transaction<'_, Postgres>,
    relation: RecordLink,
    source: &str,
    target: &str,
) -> Result<(), DbError> {
    sqlx::query(&format!(
        "INSERT INTO {RECORD_LINKS_TABLE} (relation, source, target) VALUES ($1, $2, $3) ON CONFLICT DO NOTHING"
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
/// `Store::rebuild_projections` drives (ADR 0010), through the same inserts as the live path. The table
/// is cleared whole, not per relation: a `tagged` row may come from any of the tag-bearing kinds.
///
/// # Errors
///
/// A [`DbError`] if reading a projection or writing the index fails.
pub(crate) async fn rebuild_index(pool: &Pool<Postgres>) -> Result<(), DbError> {
    let links = all_links(pool).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(backend("opening a record links transaction"))?;
    sqlx::query(&format!("DELETE FROM {RECORD_LINKS_TABLE}"))
        .execute(&mut *tx)
        .await
        .map_err(backend("clearing the record links"))?;
    for (relation, source, target) in &links {
        insert_link(&mut tx, *relation, source, target).await?;
    }
    tx.commit().await.map_err(backend("committing the record links"))
}

/// Every `(source, target)` of `relation` whose target is one of `targets`, in that order.
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn linking(
    pool: &Pool<Postgres>,
    relation: RecordLink,
    targets: &[String],
) -> Result<Vec<(String, String)>, DbError> {
    if targets.is_empty() {
        return Ok(Vec::new());
    }
    let rows = sqlx::query(&format!(
        "SELECT source, target FROM {RECORD_LINKS_TABLE} \
         WHERE relation = $1 AND target = ANY($2)"
    ))
    .bind(relation.as_str())
    .bind(targets)
    .fetch_all(pool)
    .await
    .map_err(backend("reading the record links"))?;
    // Sorted here, as the SQLite twin sorts.
    let mut links: Vec<(String, String)> = rows.iter().map(|row| (row.get("source"), row.get("target"))).collect();
    links.sort_unstable();
    Ok(links)
}
