//! The Postgres half of the identity cluster index (ADR 0039 §4) — see the [module header](super)
//! for what the two tables hold and why the index exists. A function-for-function twin of
//! [`sqlite`](super::sqlite), except that each writer first locks `identity_edges` ([`lock_index`]):
//! SQLite serialises writers at the file, while Postgres would let two concurrent writers each rebuild
//! the closure without the other's edge.

use std::marker::PhantomData;

use async_trait::async_trait;
use cqrs_es::{EventEnvelope, Query};
use sqlx::{Pool, Postgres, Row};
use vitni_core::event::EventView;
use vitni_core::family::FamilyView;
use vitni_core::identity::ClusterRecord;
use vitni_core::matching::MatchableKind;
use vitni_core::person::PersonView;

use super::{IDENTITY_EDGES_TABLE, IDENTITY_LINKS_TABLE, IndexedRecord, closure};
use crate::postgres_query;
use crate::store::{DbError, IdentityLink};

const CREATE_IDENTITY_EDGES_TABLE: &str = "
CREATE TABLE IF NOT EXISTS identity_edges (
    kind       TEXT NOT NULL,
    surviving  TEXT NOT NULL,
    member     TEXT NOT NULL,
    PRIMARY KEY (kind, surviving, member)
)";

const CREATE_IDENTITY_LINKS_TABLE: &str = "
CREATE TABLE IF NOT EXISTS identity_links (
    kind    TEXT NOT NULL,
    member  TEXT NOT NULL,
    root    TEXT NOT NULL,
    PRIMARY KEY (kind, member)
)";

/// Wraps a `sqlx` error with what the index was doing.
fn backend(action: &'static str) -> impl Fn(sqlx::Error) -> DbError {
    move |error| DbError::Backend(format!("{action}: {error}"))
}

/// Creates the identity index tables. Idempotent. Returns whether they are new, so a workspace whose
/// log predates them gets them filled from its projections.
///
/// # Errors
///
/// Returns the `sqlx` error if a statement fails.
pub(crate) async fn create_tables(pool: &Pool<Postgres>) -> Result<bool, sqlx::Error> {
    let existed: bool = sqlx::query("SELECT to_regclass($1) IS NOT NULL AS existed")
        .bind(IDENTITY_LINKS_TABLE)
        .fetch_one(pool)
        .await?
        .get("existed");
    sqlx::query(CREATE_IDENTITY_EDGES_TABLE).execute(pool).await?;
    sqlx::query(CREATE_IDENTITY_LINKS_TABLE).execute(pool).await?;
    Ok(!existed)
}

/// A `cqrs-es` query that keeps one kind's edges and clusters in step with its projection. Must be
/// appended *after* that aggregate's `GenericQuery`, so the projection it reads is already up to date.
pub(crate) struct IdentityLinksQuery<V> {
    pool: Pool<Postgres>,
    view: PhantomData<fn() -> V>,
}

impl<V> IdentityLinksQuery<V> {
    /// Wraps the pool the projection and index tables share.
    pub(crate) fn new(pool: Pool<Postgres>) -> Self {
        Self {
            pool,
            view: PhantomData,
        }
    }
}

#[async_trait]
impl<V: IndexedRecord> Query<V::State> for IdentityLinksQuery<V> {
    async fn dispatch(&self, aggregate_id: &str, events: &[EventEnvelope<V::State>]) {
        if !events.iter().any(|envelope| V::changes_edges(&envelope.payload)) {
            return;
        }
        if let Err(error) = reindex_survivor::<V>(&self.pool, aggregate_id).await {
            tracing::error!(kind = V::KIND.as_str(), aggregate_id, %error, "failed to update the identity index");
        }
    }
}

/// Mirrors one survivor's live merge edges from its projection, then recomputes its kind's clusters.
async fn reindex_survivor<V: IndexedRecord>(pool: &Pool<Postgres>, surviving: &str) -> Result<(), DbError> {
    let kind = V::KIND.as_str();
    let mut tx = pool
        .begin()
        .await
        .map_err(backend("opening an identity index transaction"))?;
    lock_index(&mut tx).await?;
    let view = postgres_query::find_view_by_id::<V>(pool, V::VIEW_TABLE, surviving).await?;
    sqlx::query(&format!(
        "DELETE FROM {IDENTITY_EDGES_TABLE} WHERE kind = $1 AND surviving = $2"
    ))
    .bind(kind)
    .bind(surviving)
    .execute(&mut *tx)
    .await
    .map_err(backend("clearing a survivor's identity edges"))?;
    if let Some(view) = view {
        insert_edges(&mut tx, kind, surviving, &view).await?;
    }
    recompute_links(&mut tx, kind).await?;
    tx.commit().await.map_err(backend("committing the identity index"))
}

/// Serialises index writers: each one replaces the whole closure from the edges it reads, so two
/// concurrent ones would otherwise each miss the other's edge, and the later one's inserts collide.
async fn lock_index(tx: &mut sqlx::Transaction<'_, Postgres>) -> Result<(), DbError> {
    sqlx::query(&format!(
        "LOCK TABLE {IDENTITY_EDGES_TABLE} IN SHARE ROW EXCLUSIVE MODE"
    ))
    .execute(&mut **tx)
    .await
    .map_err(backend("locking the identity index"))?;
    Ok(())
}

/// Inserts one edge per record `view` has merged.
async fn insert_edges<V: ClusterRecord>(
    tx: &mut sqlx::Transaction<'_, Postgres>,
    kind: &str,
    surviving: &str,
    view: &V,
) -> Result<(), DbError> {
    for member in view.merged() {
        sqlx::query(&format!(
            "INSERT INTO {IDENTITY_EDGES_TABLE} (kind, surviving, member) VALUES ($1, $2, $3) \
             ON CONFLICT DO NOTHING"
        ))
        .bind(kind)
        .bind(surviving)
        .bind(member.to_string())
        .execute(&mut **tx)
        .await
        .map_err(backend("inserting an identity edge"))?;
    }
    Ok(())
}

/// Replaces `kind`'s closure with the one computed from its current edges.
async fn recompute_links(tx: &mut sqlx::Transaction<'_, Postgres>, kind: &str) -> Result<(), DbError> {
    let rows = sqlx::query(&format!(
        "SELECT surviving, member FROM {IDENTITY_EDGES_TABLE} WHERE kind = $1"
    ))
    .bind(kind)
    .fetch_all(&mut **tx)
    .await
    .map_err(backend("reading the identity edges"))?;
    let edges: Vec<(String, String)> = rows
        .iter()
        .map(|row| (row.get("surviving"), row.get("member")))
        .collect();
    sqlx::query(&format!("DELETE FROM {IDENTITY_LINKS_TABLE} WHERE kind = $1"))
        .bind(kind)
        .execute(&mut **tx)
        .await
        .map_err(backend("clearing the identity links"))?;
    for (member, root) in closure(&edges) {
        sqlx::query(&format!(
            "INSERT INTO {IDENTITY_LINKS_TABLE} (kind, member, root) VALUES ($1, $2, $3)"
        ))
        .bind(kind)
        .bind(member)
        .bind(root)
        .execute(&mut **tx)
        .await
        .map_err(backend("inserting an identity link"))?;
    }
    Ok(())
}

/// Rebuilds the whole index from every matchable kind's (already-rebuilt) projection — the maintenance
/// path `Store::rebuild_projections` drives (ADR 0010), through the same inserts as the live path.
///
/// # Errors
///
/// A [`DbError`] if reading a projection or writing the index fails.
pub(crate) async fn rebuild_index(pool: &Pool<Postgres>) -> Result<(), DbError> {
    rebuild_kind::<PersonView>(pool).await?;
    rebuild_kind::<EventView>(pool).await?;
    rebuild_kind::<FamilyView>(pool).await
}

/// Rebuilds one kind's edges and clusters from its projection.
async fn rebuild_kind<V: IndexedRecord>(pool: &Pool<Postgres>) -> Result<(), DbError> {
    let kind = V::KIND.as_str();
    let views: Vec<V> = postgres_query::list_views(pool, V::VIEW_TABLE).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(backend("opening an identity index transaction"))?;
    lock_index(&mut tx).await?;
    sqlx::query(&format!("DELETE FROM {IDENTITY_EDGES_TABLE} WHERE kind = $1"))
        .bind(kind)
        .execute(&mut *tx)
        .await
        .map_err(backend("clearing the identity edges"))?;
    for view in &views {
        let Some(id) = view.record_id() else { continue };
        insert_edges(&mut tx, kind, &id.to_string(), view).await?;
    }
    recompute_links(&mut tx, kind).await?;
    tx.commit().await.map_err(backend("committing the identity index"))
}

/// Every member of a `kind` cluster with its root, ordered by member.
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn links(pool: &Pool<Postgres>, kind: MatchableKind) -> Result<Vec<IdentityLink>, DbError> {
    let rows = sqlx::query(&format!(
        "SELECT member, root FROM {IDENTITY_LINKS_TABLE} WHERE kind = $1 ORDER BY member"
    ))
    .bind(kind.as_str())
    .fetch_all(pool)
    .await
    .map_err(backend("reading the identity links"))?;
    Ok(rows
        .iter()
        .map(|row| IdentityLink {
            member: row.get("member"),
            root: row.get("root"),
        })
        .collect())
}
