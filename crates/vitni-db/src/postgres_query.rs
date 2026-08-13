//! Read-model queries over the conclusion projections — private Postgres internals.
//!
//! The Postgres twin of [`crate::sqlite_query`]: same queries, same `{ "state": { … } }` payload
//! shape, but Postgres SQL. Every view table carries a `human_id` column, `GENERATED ALWAYS ...
//! STORED` from `payload->'state'->>'human_id'` (ADR 0032, superseding the inline json-operator
//! scans ADR 0009 originally fixed), indexed for every aggregate but Tag
//! (`HUMAN_ID_VIEW_TABLES`). Rows are fetched as `payload::text` to reuse the engine-neutral
//! [`deserialize_view`](crate::store::deserialize_view). Placeholders are `$1`. These functions are
//! `pub(crate)`; the engine-neutral surface is [`crate::store::Store`].

use serde::de::DeserializeOwned;
use sqlx::{PgConnection, Pool, Postgres, Row};
use vitni_core::id_format::IdFormat;

use crate::store::{DbError, StoredEvent, deserialize_view};

/// How many candidate ids a `next_human_id` length-group page fetches at a time — the Postgres
/// twin of [`crate::sqlite_query::NEXT_HUMAN_ID_PAGE_SIZE`].
const NEXT_HUMAN_ID_PAGE_SIZE: usize = 32;

/// Returns the next free `human_id` for `format` in `table` (e.g. `I0001`, then `I0002`) — the
/// Postgres twin of [`crate::sqlite_query::next_human_id`]: groups the indexed `human_id` values by
/// length and takes the numeric max within each group via [`max_number_in_length_group`], then the
/// max across groups. Grouping first is required because `IdFormat::extract_number` does not check
/// digit count, so a lexical scan across mixed widths would hand back a non-maximal number. An
/// empty projection (or none matching the format) yields the first id. Runs on one acquired
/// connection, per `_human_id_len_idx`'s per-length-group shape. Every query it issues is an index
/// probe, so its cost tracks the number of distinct id widths, not the row count (issue #233).
pub(crate) async fn next_human_id(pool: &Pool<Postgres>, table: &str, format: &IdFormat) -> Result<String, DbError> {
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

/// Returns every distinct `human_id` length present in `table`, longest first — the Postgres twin of
/// [`crate::sqlite_query::human_id_lengths_descending`], and for the same reason: one index probe per
/// distinct length rather than `SELECT DISTINCT length(human_id)`, which reads the answer off every
/// row and would leave the allocator O(rows).
async fn human_id_lengths_descending(conn: &mut PgConnection, table: &str) -> Result<Vec<i32>, DbError> {
    let mut lengths = Vec::new();
    let mut shorter_than: Option<i32> = None;
    loop {
        let sql = match shorter_than {
            None => format!(
                "SELECT length(human_id) AS len FROM {table} WHERE human_id IS NOT NULL \
                 ORDER BY length(human_id) DESC LIMIT 1"
            ),
            Some(_) => format!(
                "SELECT length(human_id) AS len FROM {table} WHERE human_id IS NOT NULL \
                 AND length(human_id) < $1 ORDER BY length(human_id) DESC LIMIT 1"
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
        let len: i32 = row.get("len");
        lengths.push(len);
        shorter_than = Some(len);
    }
}

/// Returns the numeric max among the `human_id`s of length `len` in `table`, or `None` if none of
/// them parse under `format` — the Postgres twin of
/// [`crate::sqlite_query::max_number_in_length_group`]. Within one length, lexical order under the
/// `"C"` collation is numeric order, so a descending scan's first parseable value is the group's
/// max. Pages in [`NEXT_HUMAN_ID_PAGE_SIZE`]-row chunks (keyset-paged on `human_id`) so a run of
/// unparseable ids longer than one page cannot hide a real max behind it.
async fn max_number_in_length_group(
    conn: &mut PgConnection,
    table: &str,
    len: i32,
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
/// below `cursor` (the previous page's last row) when given. `COLLATE "C"` on every comparison and
/// the `ORDER BY` matches `_human_id_len_idx`'s collation, so Postgres can use the index.
async fn fetch_length_group_page(
    conn: &mut PgConnection,
    table: &str,
    len: i32,
    cursor: Option<&str>,
) -> Result<Vec<String>, DbError> {
    let sql = match cursor {
        None => format!(
            "SELECT human_id FROM {table} WHERE length(human_id) = $1 \
             ORDER BY human_id COLLATE \"C\" DESC LIMIT {NEXT_HUMAN_ID_PAGE_SIZE}"
        ),
        Some(_) => format!(
            "SELECT human_id FROM {table} WHERE length(human_id) = $1 AND human_id COLLATE \"C\" < $2 \
             ORDER BY human_id COLLATE \"C\" DESC LIMIT {NEXT_HUMAN_ID_PAGE_SIZE}"
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
    pool: &Pool<Postgres>,
    table: &str,
    human_id: &str,
) -> Result<Option<V>, DbError> {
    let sql = format!("SELECT payload::text AS payload FROM {table} WHERE human_id = $1");
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
/// The Postgres twin of [`crate::sqlite_query::find_view_by_external_id`]: walks the `external_ids` array
/// with `json_array_elements` and reads each element's nested `value->>'authority'` /
/// `value->>'value'`. The re-import resolution key (data-model §11).
pub(crate) async fn find_view_by_external_id<V: DeserializeOwned>(
    pool: &Pool<Postgres>,
    table: &str,
    authority: &str,
    value: &str,
) -> Result<Option<V>, DbError> {
    let sql = format!(
        "SELECT payload::text AS payload FROM {table} WHERE EXISTS (\
         SELECT 1 FROM json_array_elements(payload->'state'->'external_ids') AS e \
         WHERE e->'value'->>'authority' = $1 AND e->'value'->>'value' = $2) LIMIT 1"
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

/// Loads every view in `table` whose `subjects` set names the subject serialized as
/// `{ "<subject_kind>": <subject_value> }`, ordered by `human_id`.
///
/// The Postgres twin of [`crate::sqlite_query::list_views_by_subject`]: walks the `subjects` array
/// with `json_array_elements` and reads each element's own tag key with `->>`. `subject_kind` is
/// code-supplied (one of `Person`/`Family`/`Event`/`Place`), never user input, so it is interpolated
/// directly like `table`.
pub(crate) async fn list_views_by_subject<V: DeserializeOwned>(
    pool: &Pool<Postgres>,
    table: &str,
    subject_kind: &str,
    subject_value: &str,
) -> Result<Vec<V>, DbError> {
    let sql = format!(
        "SELECT payload::text AS payload FROM {table} WHERE EXISTS (\
         SELECT 1 FROM json_array_elements(payload->'state'->'subjects') AS e \
         WHERE e->>'{subject_kind}' = $1) \
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
    let sql = format!("SELECT payload::text AS payload FROM {table} ORDER BY human_id");
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

/// Reads the raw event stream for one aggregate instance, ordered by `sequence` ascending — the
/// Postgres twin of [`crate::sqlite_query::read_aggregate_events`] (Phase 5 PR 5).
pub(crate) async fn read_aggregate_events(
    pool: &Pool<Postgres>,
    aggregate_type: &str,
    aggregate_id: &str,
) -> Result<Vec<StoredEvent>, DbError> {
    let rows = sqlx::query(
        "SELECT aggregate_type, aggregate_id, sequence, event_type, payload::text AS payload \
         FROM events WHERE aggregate_type = $1 AND aggregate_id = $2 ORDER BY sequence ASC",
    )
    .bind(aggregate_type)
    .bind(aggregate_id)
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(rows.iter().map(stored_event).collect())
}

/// Reads the most recent events across every aggregate, newest first by the in-payload
/// `occurred_at` — the Postgres twin of [`crate::sqlite_query::read_recent_events`].
pub(crate) async fn read_recent_events(pool: &Pool<Postgres>, limit: u32) -> Result<Vec<StoredEvent>, DbError> {
    let rows = sqlx::query(
        "SELECT aggregate_type, aggregate_id, sequence, event_type, payload::text AS payload \
         FROM events ORDER BY payload->'context'->>'occurred_at' DESC, sequence DESC LIMIT $1",
    )
    .bind(i64::from(limit))
    .fetch_all(pool)
    .await
    .map_err(|e| DbError::Backend(e.to_string()))?;
    Ok(rows.iter().map(stored_event).collect())
}

/// Maps each `view_id` in `table` to its `human_id`, skipping rows without one — the Postgres twin
/// of [`crate::sqlite_query::human_id_index`].
pub(crate) async fn human_id_index(pool: &Pool<Postgres>, table: &str) -> Result<Vec<(String, String)>, DbError> {
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

/// Counts the rows (aggregate instances) in `table` — the Postgres twin of
/// [`crate::sqlite_query::count_rows`].
pub(crate) async fn count_rows(pool: &Pool<Postgres>, table: &str) -> Result<u64, DbError> {
    let sql = format!("SELECT COUNT(*) AS n FROM {table}");
    let row = sqlx::query(&sql)
        .fetch_one(pool)
        .await
        .map_err(|e| DbError::Backend(e.to_string()))?;
    let count: i64 = row.get("n");
    Ok(count.unsigned_abs())
}

/// Builds a [`StoredEvent`] from an `events` row (shared by the stream/recent reads).
fn stored_event(row: &sqlx::postgres::PgRow) -> StoredEvent {
    StoredEvent {
        aggregate_type: row.get("aggregate_type"),
        aggregate_id: row.get("aggregate_id"),
        sequence: row.get("sequence"),
        event_type: row.get("event_type"),
        payload: row.get("payload"),
    }
}
