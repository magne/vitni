//! The SQLite half of the identity cluster index (ADR 0039 §4) — see the [module header](super) for
//! what the two tables hold and why the index exists.

use async_trait::async_trait;
use cqrs_es::{EventEnvelope, Query};
use sqlx::{Pool, Row, Sqlite};
use vitni_core::matching::MatchableKind;
use vitni_core::person::{PersonState, PersonView};

use super::{IDENTITY_EDGES_TABLE, IDENTITY_LINKS_TABLE, changes_edges, closure};
use crate::sqlite_query;
use crate::store::{DbError, IdentityLink};
use crate::tables::PERSON_VIEW_TABLE;

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
pub(crate) async fn create_tables(pool: &Pool<Sqlite>) -> Result<bool, sqlx::Error> {
    let existed = sqlx::query("SELECT 1 FROM sqlite_master WHERE type = 'table' AND name = ?")
        .bind(IDENTITY_LINKS_TABLE)
        .fetch_optional(pool)
        .await?
        .is_some();
    sqlx::query(CREATE_IDENTITY_EDGES_TABLE).execute(pool).await?;
    sqlx::query(CREATE_IDENTITY_LINKS_TABLE).execute(pool).await?;
    Ok(!existed)
}

/// A `cqrs-es` query that keeps the person edges and clusters in step with the Person projection. Must
/// be appended *after* the person `GenericQuery`, so the projection it reads is already up to date.
pub(crate) struct IdentityLinksQuery {
    pool: Pool<Sqlite>,
}

impl IdentityLinksQuery {
    /// Wraps the pool the projection and index tables share.
    pub(crate) fn new(pool: Pool<Sqlite>) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Query<PersonState> for IdentityLinksQuery {
    async fn dispatch(&self, aggregate_id: &str, events: &[EventEnvelope<PersonState>]) {
        if !events.iter().any(|envelope| changes_edges(&envelope.payload)) {
            return;
        }
        if let Err(error) = reindex_survivor(&self.pool, aggregate_id).await {
            tracing::error!(person_id = aggregate_id, %error, "failed to update the identity index");
        }
    }
}

/// Mirrors one survivor's live merge edges from its projection, then recomputes the person clusters.
async fn reindex_survivor(pool: &Pool<Sqlite>, surviving: &str) -> Result<(), DbError> {
    let kind = MatchableKind::Person.as_str();
    let view = sqlite_query::find_view_by_id::<PersonView>(pool, PERSON_VIEW_TABLE, surviving).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(backend("opening an identity index transaction"))?;
    sqlx::query(&format!(
        "DELETE FROM {IDENTITY_EDGES_TABLE} WHERE kind = ? AND surviving = ?"
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

/// Inserts one edge per record `view` has merged.
async fn insert_edges(
    tx: &mut sqlx::Transaction<'_, Sqlite>,
    kind: &str,
    surviving: &str,
    view: &PersonView,
) -> Result<(), DbError> {
    for member in view.merged() {
        sqlx::query(&format!(
            "INSERT OR IGNORE INTO {IDENTITY_EDGES_TABLE} (kind, surviving, member) VALUES (?, ?, ?)"
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
async fn recompute_links(tx: &mut sqlx::Transaction<'_, Sqlite>, kind: &str) -> Result<(), DbError> {
    let rows = sqlx::query(&format!(
        "SELECT surviving, member FROM {IDENTITY_EDGES_TABLE} WHERE kind = ?"
    ))
    .bind(kind)
    .fetch_all(&mut **tx)
    .await
    .map_err(backend("reading the identity edges"))?;
    let edges: Vec<(String, String)> = rows
        .iter()
        .map(|row| (row.get("surviving"), row.get("member")))
        .collect();
    sqlx::query(&format!("DELETE FROM {IDENTITY_LINKS_TABLE} WHERE kind = ?"))
        .bind(kind)
        .execute(&mut **tx)
        .await
        .map_err(backend("clearing the identity links"))?;
    for (member, root) in closure(&edges) {
        sqlx::query(&format!(
            "INSERT INTO {IDENTITY_LINKS_TABLE} (kind, member, root) VALUES (?, ?, ?)"
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

/// Rebuilds the whole index from every person's (already-rebuilt) projection — the maintenance path
/// `Store::rebuild_projections` drives (ADR 0010), through the same inserts as the live path.
///
/// # Errors
///
/// A [`DbError`] if reading a projection or writing the index fails.
pub(crate) async fn rebuild_index(pool: &Pool<Sqlite>) -> Result<(), DbError> {
    let kind = MatchableKind::Person.as_str();
    let views: Vec<PersonView> = sqlite_query::list_views(pool, PERSON_VIEW_TABLE).await?;
    let mut tx = pool
        .begin()
        .await
        .map_err(backend("opening an identity index transaction"))?;
    sqlx::query(&format!("DELETE FROM {IDENTITY_EDGES_TABLE} WHERE kind = ?"))
        .bind(kind)
        .execute(&mut *tx)
        .await
        .map_err(backend("clearing the identity edges"))?;
    for view in &views {
        let Some(person_id) = view.person_id() else { continue };
        insert_edges(&mut tx, kind, &person_id.to_string(), view).await?;
    }
    recompute_links(&mut tx, kind).await?;
    tx.commit().await.map_err(backend("committing the identity index"))
}

/// Every member of a `kind` cluster with its root, ordered by member.
///
/// # Errors
///
/// A [`DbError`] if the query fails.
pub(crate) async fn links(pool: &Pool<Sqlite>, kind: MatchableKind) -> Result<Vec<IdentityLink>, DbError> {
    let rows = sqlx::query(&format!(
        "SELECT member, root FROM {IDENTITY_LINKS_TABLE} WHERE kind = ? ORDER BY member"
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
