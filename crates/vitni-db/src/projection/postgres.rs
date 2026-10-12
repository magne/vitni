//! The Postgres view rows of the [projection writer](super).

use async_trait::async_trait;
use sqlx::{Pool, Postgres, Row};

use super::{StoredView, ViewRows, Written};
use crate::store::DbError;

#[async_trait]
impl ViewRows for Pool<Postgres> {
    async fn load(&self, table: &str, view_id: &str) -> Result<Option<StoredView>, DbError> {
        let row = sqlx::query(&format!(
            "SELECT sequence, payload::text AS payload FROM {table} WHERE view_id = $1"
        ))
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
        let written = match read {
            None => sqlx::query(&format!(
                "INSERT INTO {table} (payload, sequence, view_id) VALUES ($1, $2, $3) ON CONFLICT (view_id) DO NOTHING"
            ))
            .bind(payload)
            .bind(sequence)
            .bind(view_id)
            .execute(self)
            .await,
            Some(read) => {
                sqlx::query(&format!(
                    "UPDATE {table} SET payload = $1, sequence = $2 WHERE view_id = $3 AND sequence = $4"
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
