//! Read-model queries over the conclusion projections — private Postgres internals.
//!
//! The Postgres twin of [`crate::query`]: same five queries, same `{ "state": { … } }` payload
//! shape, but Postgres SQL. The `postgres-es` view tables store `payload` as a `json` column, so
//! the user-facing identifier is read with the json path operators (`payload->'state'->>'human_id'`)
//! and the row is fetched as `payload::text` to reuse the engine-neutral
//! [`deserialize_view`](crate::store::deserialize_view). Placeholders are `$1`. These functions are
//! `pub(crate)`; the engine-neutral surface is [`crate::store::Store`].

use genealogy_core::id_format::IdFormat;
use serde::de::DeserializeOwned;
use sqlx::{Pool, Postgres, Row};

use crate::store::{DbError, deserialize_view};

/// Returns the next free `human_id` for `format` in `table` (e.g. `I0001`, then `I0002`).
///
/// Reads every stored `human_id`, extracts each id's numeric part with the format, takes the
/// maximum, and renders `max + 1` — numerically, so width growth (`I9999` → `I10000`) and arbitrary
/// prefix/suffix patterns stay correct. An empty projection yields the first id.
pub(crate) async fn next_human_id(pool: &Pool<Postgres>, table: &str, format: &IdFormat) -> Result<String, DbError> {
    let sql = format!("SELECT payload->'state'->>'human_id' AS human_id FROM {table}");
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;

    let mut highest = 0u64;
    let mut seen = false;
    for row in rows {
        let stored: Option<String> = row.get("human_id");
        let Some(stored) = stored else { continue };
        if let Some(number) = format.extract_number(&stored) {
            seen = true;
            highest = highest.max(number);
        }
    }

    let next = if seen { highest + 1 } else { 1 };
    Ok(format.render(next))
}

/// Loads the view in `table` whose `human_id` equals `human_id`, if any.
pub(crate) async fn find_view_by_human_id<V: DeserializeOwned>(
    pool: &Pool<Postgres>,
    table: &str,
    human_id: &str,
) -> Result<Option<V>, DbError> {
    let sql = format!("SELECT payload::text AS payload FROM {table} WHERE payload->'state'->>'human_id' = $1");
    let row = sqlx::query(&sql)
        .bind(human_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;

    let Some(row) = row else {
        return Ok(None);
    };
    let payload: String = row.get("payload");
    Ok(Some(deserialize_view(table, &payload)?))
}

/// Loads the view in `table` whose `view_id` (the aggregate id PK) equals `view_id`, if any.
///
/// Used for aggregates without a `HumanId` (the Tag definition — data-model §9), which are looked
/// up by their aggregate id rather than a user-facing id.
pub(crate) async fn find_view_by_id<V: DeserializeOwned>(
    pool: &Pool<Postgres>,
    table: &str,
    view_id: &str,
) -> Result<Option<V>, DbError> {
    let sql = format!("SELECT payload::text AS payload FROM {table} WHERE view_id = $1");
    let row = sqlx::query(&sql)
        .bind(view_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;

    let Some(row) = row else {
        return Ok(None);
    };
    let payload: String = row.get("payload");
    Ok(Some(deserialize_view(table, &payload)?))
}

/// Loads every view in `table`, ordered by `human_id`.
pub(crate) async fn list_views<V: DeserializeOwned>(pool: &Pool<Postgres>, table: &str) -> Result<Vec<V>, DbError> {
    let sql = format!("SELECT payload::text AS payload FROM {table} ORDER BY payload->'state'->>'human_id'");
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;

    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        let payload: String = row.get("payload");
        views.push(deserialize_view(table, &payload)?);
    }
    Ok(views)
}

/// Returns whether a view with primary key `view_id` exists in `table` — the by-id existence check
/// the cross-aggregate invariant checks read (ADR 0009 §2; ADR 0004 §3). `view_id` is the
/// aggregate id, the table's primary key, so this is an indexed point lookup.
pub(crate) async fn view_exists(pool: &Pool<Postgres>, table: &str, view_id: &str) -> Result<bool, DbError> {
    let sql = format!("SELECT 1 FROM {table} WHERE view_id = $1");
    let row = sqlx::query(&sql)
        .bind(view_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(row.is_some())
}
