//! Read-model queries over the conclusion projections — private SQLite internals.
//!
//! Each aggregate's projection is one row per instance in its `*_view` table
//! (`view_id, version, payload, human_id`), where `payload` is the serialized view and `human_id`
//! is a `GENERATED ALWAYS ... STORED` column mirroring the JSON path `$.state.human_id` (ADR 0032,
//! superseding the `json_extract` scans ADR 0009 originally fixed), indexed for every aggregate
//! but Tag (`HUMAN_ID_VIEW_TABLES`). The queries are generic over the view type and parameterized
//! by the (code-supplied, trusted) table name, so every aggregate reuses one implementation. These
//! functions are `pub(crate)`; the engine-neutral surface is [`crate::store::Store`].

use serde::de::DeserializeOwned;
use sqlx::{Pool, Row, Sqlite, SqliteConnection};
use vitni_core::id_format::IdFormat;

use crate::store::{DbError, StoredEvent};

/// How many candidate ids a `next_human_id` length-group page fetches at a time; a page is
/// re-fetched (with the last row as the new keyset cursor) only when every id in it fails to
/// parse under `format` — junk this deep is not the expected case.
const NEXT_HUMAN_ID_PAGE_SIZE: usize = 32;

/// Returns the next free `human_id` for `format` in `table` (e.g. `I0001`, then `I0002`).
///
/// Groups the indexed `human_id` values by length and takes the numeric max within each group via
/// [`max_number_in_length_group`], then the max across groups — grouping first because
/// `IdFormat::extract_number` does not check digit count, so a lexical/descending scan across
/// mixed widths (`I00000003` sorts after `I10001`) would hand back a number that is not the true
/// maximum. Working numerically (not lexicographically) also keeps allocation correct across
/// width growth (`I9999` → `I10000`). An empty projection (or none matching the format) yields the
/// first id. Runs on one acquired connection, per `_human_id_len_idx`'s per-length-group shape.
///
/// Every query it issues is an index probe over `_human_id_len_idx`, so the cost tracks how many
/// distinct id widths a workspace holds (one, normally) rather than how many rows the projection
/// has — the point of issue #233.
pub(crate) async fn next_human_id(pool: &Pool<Sqlite>, table: &str, format: &IdFormat) -> Result<String, DbError> {
    let mut conn = pool.acquire().await.map_err(|e| DbError::Backend(e.to_string()))?;
    let lengths = human_id_lengths_descending(&mut conn, table).await?;

    let mut highest: Option<u64> = None;
    for len in lengths {
        if let Some(candidate) = max_number_in_length_group(&mut conn, table, len, format).await? {
            highest = Some(highest.map_or(candidate, |current| current.max(candidate)));
        }
    }
    Ok(format.render(highest.map_or(1, |max| max + 1)))
}

/// Returns every distinct `human_id` length present in `table`, longest first — the group boundaries
/// [`next_human_id`] takes its per-group max within.
///
/// Walks the lengths one at a time, each step an index probe for "the longest id shorter than the
/// last one", so it reads one row per distinct length. `SELECT DISTINCT length(human_id)` reads the
/// same answer off *every* row instead: SQLite has no loose index scan, so it walks the whole index
/// (measured at 50 000 rows: 1.745 ms against 0.010 ms for one probe here), which would leave the
/// allocator O(rows) — the scan issue #233 exists to remove.
async fn human_id_lengths_descending(conn: &mut SqliteConnection, table: &str) -> Result<Vec<i64>, DbError> {
    let mut lengths = Vec::new();
    let mut shorter_than: Option<i64> = None;
    loop {
        let sql = match shorter_than {
            None => format!(
                "SELECT length(human_id) AS len FROM {table} WHERE human_id IS NOT NULL \
                 ORDER BY length(human_id) DESC LIMIT 1"
            ),
            Some(_) => format!(
                "SELECT length(human_id) AS len FROM {table} WHERE human_id IS NOT NULL \
                 AND length(human_id) < ? ORDER BY length(human_id) DESC LIMIT 1"
            ),
        };
        let mut query = sqlx::query(&sql);
        if let Some(bound) = shorter_than {
            query = query.bind(bound);
        }
        let row = query
            .fetch_optional(&mut *conn)
            .await
            .map_err(|e| DbError::Backend(e.to_string()))?;
        let Some(row) = row else {
            return Ok(lengths);
        };
        let len: i64 = row.get("len");
        lengths.push(len);
        shorter_than = Some(len);
    }
}

/// Returns the numeric max among the `human_id`s of length `len` in `table`, or `None` if none of
/// them parse under `format`.
///
/// Within one length, lexical order is numeric order, so a descending scan's first parseable value
/// is the group's max — no need to read the whole group. Pages in
/// [`NEXT_HUMAN_ID_PAGE_SIZE`]-row chunks (keyset-paged on `human_id`) so a run of unparseable ids
/// longer than one page cannot hide a real max behind it.
async fn max_number_in_length_group(
    conn: &mut SqliteConnection,
    table: &str,
    len: i64,
    format: &IdFormat,
) -> Result<Option<u64>, DbError> {
    let mut cursor: Option<String> = None;
    loop {
        let page = fetch_length_group_page(conn, table, len, cursor.as_deref()).await?;
        for human_id in &page {
            if let Some(number) = format.extract_number(human_id) {
                return Ok(Some(number));
            }
        }
        if page.len() < NEXT_HUMAN_ID_PAGE_SIZE {
            return Ok(None);
        }
        cursor = page.into_iter().next_back();
    }
}

/// Fetches one descending, keyset-paged page of `human_id`s of length `len` in `table`, strictly
/// below `cursor` (the previous page's last row) when given.
async fn fetch_length_group_page(
    conn: &mut SqliteConnection,
    table: &str,
    len: i64,
    cursor: Option<&str>,
) -> Result<Vec<String>, DbError> {
    let sql = match cursor {
        None => format!(
            "SELECT human_id FROM {table} WHERE length(human_id) = ? \
             ORDER BY human_id DESC LIMIT {NEXT_HUMAN_ID_PAGE_SIZE}"
        ),
        Some(_) => format!(
            "SELECT human_id FROM {table} WHERE length(human_id) = ? AND human_id < ? \
             ORDER BY human_id DESC LIMIT {NEXT_HUMAN_ID_PAGE_SIZE}"
        ),
    };
    let mut query = sqlx::query(&sql).bind(len);
    if let Some(cursor) = cursor {
        query = query.bind(cursor);
    }
    let rows = query
        .fetch_all(&mut *conn)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(rows.iter().map(|row| row.get("human_id")).collect())
}

/// Loads the view in `table` whose `human_id` equals `human_id`, if any.
pub(crate) async fn find_view_by_human_id<V: DeserializeOwned>(
    pool: &Pool<Sqlite>,
    table: &str,
    human_id: &str,
) -> Result<Option<V>, DbError> {
    let sql = format!("SELECT payload FROM {table} WHERE human_id = ?");
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
/// [`ExternalId`](vitni_core::text::ExternalId)) under `$.state`, so the match walks the array
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

/// Loads every view in `table` whose `subjects` set (a `BTreeSet<SubjectRef>` — ADR 0028 §2) names
/// the subject serialized as `{ "<subject_kind>": <subject_value> }`, ordered by `human_id`.
///
/// `subjects` is an array of externally-tagged `SubjectRef` values under `$.state.subjects`, so the
/// match walks it with `json_each` and reads each element's own tag key — the reverse-by-subject
/// index ("which research notes argue about this Person/Family/Event/Place", ADR 0028 §5), the same
/// array-walk shape as [`find_view_by_external_id`]. `subject_kind` is code-supplied (one of
/// `Person`/`Family`/`Event`/`Place`), never user input, so it is interpolated directly like `table`.
pub(crate) async fn list_views_by_subject<V: DeserializeOwned>(
    pool: &Pool<Sqlite>,
    table: &str,
    subject_kind: &str,
    subject_value: &str,
) -> Result<Vec<V>, DbError> {
    let sql = format!(
        "SELECT payload FROM {table} WHERE EXISTS (\
         SELECT 1 FROM json_each(payload, '$.state.subjects') je \
         WHERE json_extract(je.value, '$.{subject_kind}') = ?) \
         ORDER BY human_id"
    );
    let rows = sqlx::query(&sql)
        .bind(subject_value)
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
    let sql = format!("SELECT payload FROM {table} ORDER BY human_id");
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

/// Reads the raw event stream for one aggregate instance, ordered by `sequence` ascending.
///
/// The audit/change-log read path (Phase 5 PR 5): unlike the projection queries, this returns the
/// immutable events themselves so the application layer can render who/when/why. The provenance
/// envelope travels in each `payload` (ADR 0004 §1).
pub(crate) async fn read_aggregate_events(
    pool: &Pool<Sqlite>,
    aggregate_type: &str,
    aggregate_id: &str,
) -> Result<Vec<StoredEvent>, DbError> {
    let rows = sqlx::query(
        "SELECT aggregate_type, aggregate_id, sequence, event_type, payload \
         FROM events WHERE aggregate_type = ? AND aggregate_id = ? ORDER BY sequence ASC",
    )
    .bind(aggregate_type)
    .bind(aggregate_id)
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(rows.iter().map(stored_event).collect())
}

/// Reads the most recent events across every aggregate, newest first, capped at `limit`.
///
/// The `events` table has no insert-time column, so recency is the in-payload `occurred_at` of the
/// provenance envelope (`$.context.occurred_at`, an RFC 3339 string that sorts lexicographically).
pub(crate) async fn read_recent_events(pool: &Pool<Sqlite>, limit: u32) -> Result<Vec<StoredEvent>, DbError> {
    let rows = sqlx::query(
        "SELECT aggregate_type, aggregate_id, sequence, event_type, payload \
         FROM events ORDER BY json_extract(payload, '$.context.occurred_at') DESC, sequence DESC LIMIT ?",
    )
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(rows.iter().map(stored_event).collect())
}

/// Maps each `view_id` (the aggregate id PK) in `table` to its `human_id`, skipping rows without one.
///
/// Lets the change-log resolve an event's `aggregate_id` (a UUID) to the user-facing id a frontend
/// links to.
pub(crate) async fn human_id_index(pool: &Pool<Sqlite>, table: &str) -> Result<Vec<(String, String)>, DbError> {
    let sql = format!("SELECT view_id, human_id FROM {table}");
    let rows = sqlx::query(&sql)
        .fetch_all(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    let mut index = Vec::with_capacity(rows.len());
    for row in rows {
        let human_id: Option<String> = row.get("human_id");
        if let Some(human_id) = human_id {
            index.push((row.get("view_id"), human_id));
        }
    }
    Ok(index)
}

/// Counts the rows (aggregate instances) in `table` — the projected-record count for a category.
pub(crate) async fn count_rows(pool: &Pool<Sqlite>, table: &str) -> Result<u64, DbError> {
    let sql = format!("SELECT COUNT(*) AS n FROM {table}");
    let row = sqlx::query(&sql)
        .fetch_one(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    let count: i64 = row.get("n");
    Ok(count.unsigned_abs())
}

/// Builds a [`StoredEvent`] from an `events` row (shared by the stream/recent reads).
fn stored_event(row: &sqlx::sqlite::SqliteRow) -> StoredEvent {
    StoredEvent {
        aggregate_type: row.get("aggregate_type"),
        aggregate_id: row.get("aggregate_id"),
        sequence: row.get("sequence"),
        event_type: row.get("event_type"),
        payload: row.get("payload"),
    }
}

/// Deserializes a stored projection payload, mapping failures to [`DbError::Backend`].
fn deserialize_view<V: DeserializeOwned>(table: &str, payload: &str) -> Result<V, DbError> {
    serde_json::from_str(payload).map_err(|e| DbError::Backend(format!("decoding {table} payload: {e}")))
}
