//! Read-model queries over the Person projection — private SQLite internals.
//!
//! The conclusion projection is one row per person in the `person_view` table
//! (`view_id, version, payload`), where `payload` is the serialized `PersonView`. `PersonView`
//! serializes as `{ "state": { … } }`, so the user-facing identifier is at the JSON path
//! `$.state.human_id` (SQLite `json_extract`). These functions are `pub(crate)`; the engine-neutral
//! surface is [`crate::store::Store`].

use genealogy_core::family::FamilyView;
use genealogy_core::id_format::IdFormat;
use genealogy_core::person::PersonView;
use sqlx::{Pool, Row, Sqlite};

use crate::sqlite::{FAMILY_VIEW_TABLE, PERSON_VIEW_TABLE};
use crate::store::DbError;

/// Returns the next free person `human_id` for `format` (e.g. `I0001`, then `I0002`).
///
/// Reads every stored `human_id`, extracts each id's numeric part with the format, takes the
/// maximum, and renders `max + 1`. Working numerically (not lexicographically) keeps allocation
/// correct across width growth (`I9999` → `I10000`) and for arbitrary prefix/suffix patterns. An
/// empty projection (or none matching the format) yields the first id.
pub(crate) async fn next_person_human_id(pool: &Pool<Sqlite>, format: &IdFormat) -> Result<String, DbError> {
    let sql = format!("SELECT json_extract(payload, '$.state.human_id') AS human_id FROM {PERSON_VIEW_TABLE}");
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

/// Loads the [`PersonView`] whose `human_id` equals `human_id`, if any.
pub(crate) async fn find_person_by_human_id(
    pool: &Pool<Sqlite>,
    human_id: &str,
) -> Result<Option<PersonView>, DbError> {
    let sql = format!("SELECT payload FROM {PERSON_VIEW_TABLE} WHERE json_extract(payload, '$.state.human_id') = ?");
    let row = sqlx::query(&sql)
        .bind(human_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;

    let Some(row) = row else {
        return Ok(None);
    };
    let payload: String = row.get("payload");
    let view = deserialize_view(&payload)?;
    Ok(Some(view))
}

/// Loads every person's [`PersonView`], ordered by `human_id`.
pub(crate) async fn list_person_views(pool: &Pool<Sqlite>) -> Result<Vec<PersonView>, DbError> {
    let sql = format!("SELECT payload FROM {PERSON_VIEW_TABLE} ORDER BY json_extract(payload, '$.state.human_id')");
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;

    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        let payload: String = row.get("payload");
        views.push(deserialize_view(&payload)?);
    }
    Ok(views)
}

/// Deserializes a stored projection payload, mapping failures to [`DbError::Backend`].
fn deserialize_view(payload: &str) -> Result<PersonView, DbError> {
    serde_json::from_str(payload).map_err(|e| DbError::Backend(format!("decoding person_view payload: {e}")))
}

/// Returns the next free family `human_id` for `format` (e.g. `F0001`, then `F0002`).
///
/// Same numeric allocation as [`next_person_human_id`], over the `family_view` projection.
pub(crate) async fn next_family_human_id(pool: &Pool<Sqlite>, format: &IdFormat) -> Result<String, DbError> {
    let sql = format!("SELECT json_extract(payload, '$.state.human_id') AS human_id FROM {FAMILY_VIEW_TABLE}");
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

/// Loads the [`FamilyView`] whose `human_id` equals `human_id`, if any.
pub(crate) async fn find_family_by_human_id(
    pool: &Pool<Sqlite>,
    human_id: &str,
) -> Result<Option<FamilyView>, DbError> {
    let sql = format!("SELECT payload FROM {FAMILY_VIEW_TABLE} WHERE json_extract(payload, '$.state.human_id') = ?");
    let row = sqlx::query(&sql)
        .bind(human_id)
        .fetch_optional(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;

    let Some(row) = row else {
        return Ok(None);
    };
    let payload: String = row.get("payload");
    let view = deserialize_family_view(&payload)?;
    Ok(Some(view))
}

/// Loads every family's [`FamilyView`], ordered by `human_id`.
pub(crate) async fn list_family_views(pool: &Pool<Sqlite>) -> Result<Vec<FamilyView>, DbError> {
    let sql = format!("SELECT payload FROM {FAMILY_VIEW_TABLE} ORDER BY json_extract(payload, '$.state.human_id')");
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;

    let mut views = Vec::with_capacity(rows.len());
    for row in rows {
        let payload: String = row.get("payload");
        views.push(deserialize_family_view(&payload)?);
    }
    Ok(views)
}

/// Deserializes a stored family projection payload, mapping failures to [`DbError::Backend`].
fn deserialize_family_view(payload: &str) -> Result<FamilyView, DbError> {
    serde_json::from_str(payload).map_err(|e| DbError::Backend(format!("decoding family_view payload: {e}")))
}
