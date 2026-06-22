//! Read-model queries over the conclusion projections — private SQLite internals.
//!
//! Each aggregate's projection is one row per instance in its `*_view` table
//! (`view_id, version, payload`), where `payload` is the serialized view. Every view serializes as
//! `{ "state": { … } }`, so the user-facing identifier is at the JSON path `$.state.human_id`
//! (SQLite `json_extract`, the secondary-lookup surface fixed by ADR 0009). The queries are generic
//! over the view type and parameterized by the (code-supplied, trusted) table name, so every
//! aggregate reuses one implementation. These functions are `pub(crate)`; the engine-neutral
//! surface is [`crate::store::Store`].

use genealogy_core::id_format::IdFormat;
use serde::de::DeserializeOwned;
use sqlx::{Pool, Row, Sqlite};

use crate::store::DbError;

/// Returns the next free `human_id` for `format` in `table` (e.g. `I0001`, then `I0002`).
///
/// Reads every stored `human_id`, extracts each id's numeric part with the format, takes the
/// maximum, and renders `max + 1`. Working numerically (not lexicographically) keeps allocation
/// correct across width growth (`I9999` → `I10000`) and for arbitrary prefix/suffix patterns. An
/// empty projection (or none matching the format) yields the first id.
pub(crate) async fn next_human_id(pool: &Pool<Sqlite>, table: &str, format: &IdFormat) -> Result<String, DbError> {
    let sql = format!("SELECT json_extract(payload, '$.state.human_id') AS human_id FROM {table}");
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
    pool: &Pool<Sqlite>,
    table: &str,
    human_id: &str,
) -> Result<Option<V>, DbError> {
    let sql = format!("SELECT payload FROM {table} WHERE json_extract(payload, '$.state.human_id') = ?");
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

/// Loads the view in `table` carrying a live external id with this `(authority, value)`, if any.
///
/// `external_ids` is an array of `{assertion_id, value: {authority, value, …}}` (an attributed
/// [`ExternalId`](genealogy_core::text::ExternalId)) under `$.state`, so the match walks the array
/// with `json_each` and reads each element's nested `value.authority` / `value.value`. This is the
/// re-import resolution key (data-model §11): an incoming record is resolved to its existing
/// aggregate instead of creating a duplicate.
pub(crate) async fn find_view_by_external_id<V: DeserializeOwned>(
    pool: &Pool<Sqlite>,
    table: &str,
    authority: &str,
    value: &str,
) -> Result<Option<V>, DbError> {
    let sql = format!(
        "SELECT payload FROM {table} WHERE EXISTS (\
         SELECT 1 FROM json_each(payload, '$.state.external_ids') je \
         WHERE json_extract(je.value, '$.value.authority') = ? \
         AND json_extract(je.value, '$.value.value') = ?) LIMIT 1"
    );
    let row = sqlx::query(&sql)
        .bind(authority)
        .bind(value)
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
    pool: &Pool<Sqlite>,
    table: &str,
    view_id: &str,
) -> Result<Option<V>, DbError> {
    let sql = format!("SELECT payload FROM {table} WHERE view_id = ?");
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
pub(crate) async fn list_views<V: DeserializeOwned>(pool: &Pool<Sqlite>, table: &str) -> Result<Vec<V>, DbError> {
    let sql = format!("SELECT payload FROM {table} ORDER BY json_extract(payload, '$.state.human_id')");
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;

    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        let payload: String = row.get("payload");
        views.push(crate::store::deserialize_view(table, &payload)?);
    }
    Ok(views)
}

/// Returns whether a view with primary key `view_id` exists in `table` — the by-id existence check
/// the cross-aggregate invariant checks read (ADR 0009 §2; ADR 0004 §3). `view_id` is the
/// aggregate id, the table's primary key, so this is an indexed point lookup.
pub(crate) async fn view_exists(pool: &Pool<Sqlite>, table: &str, view_id: &str) -> Result<bool, DbError> {
    let sql = format!("SELECT 1 FROM {table} WHERE view_id = ?");
    let row = sqlx::query(&sql)
        .bind(view_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(row.is_some())
}

/// Deserializes a stored projection payload, mapping failures to [`DbError::Backend`].
fn deserialize_view<V: DeserializeOwned>(table: &str, payload: &str) -> Result<V, DbError> {
    serde_json::from_str(payload).map_err(|e| DbError::Backend(format!("decoding {table} payload: {e}")))
}
