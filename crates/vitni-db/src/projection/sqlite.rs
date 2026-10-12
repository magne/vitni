//! The SQLite view rows of the [projection writer](super).

use async_trait::async_trait;
use sqlx::{Pool, Row, Sqlite};

use super::{StoredView, ViewRows, Written};
use crate::store::DbError;

#[async_trait]
impl ViewRows for Pool<Sqlite> {
    async fn load(&self, table: &str, view_id: &str) -> Result<Option<StoredView>, DbError> {
        let row = sqlx::query(&format!("SELECT sequence, payload FROM {table} WHERE view_id = ?"))
            .bind(view_id)
            .fetch_optional(self)
            .await
            .map_err(|e| DbError::Backend(format!("reading {table} {view_id}: {e}")))?;
        let Some(row) = row else {
            return Ok(None);
        };
        let sequence: i64 = row.get("sequence");
        Ok(Some(StoredView {
            payload: row.get("payload"),
            sequence: super::sequence_from(table, view_id, sequence)?,
        }))
    }

    async fn write(
        &self,
        table: &str,
        view_id: &str,
        payload: serde_json::Value,
        sequence: usize,
        read: Option<usize>,
    ) -> Result<Written, DbError> {
        let sequence = super::sequence_to(sequence)?;
        let written =
            match read {
                None => sqlx::query(&format!(
                    "INSERT INTO {table} (payload, sequence, view_id) VALUES (?, ?, ?) ON CONFLICT (view_id) DO NOTHING"
                ))
                .bind(payload)
                .bind(sequence)
                .bind(view_id)
                .execute(self)
                .await,
                Some(read) => {
                    sqlx::query(&format!(
                        "UPDATE {table} SET payload = ?, sequence = ? WHERE view_id = ? AND sequence = ?"
                    ))
                    .bind(payload)
                    .bind(sequence)
                    .bind(view_id)
                    .bind(super::sequence_to(read)?)
                    .execute(self)
                    .await
                }
            }
            .map_err(|e| DbError::Backend(format!("writing {table} {view_id}: {e}")))?;
        Ok(if written.rows_affected() == 0 {
            Written::Lost
        } else {
            Written::Landed
        })
    }
}
