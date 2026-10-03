use chrono::Utc;
use sqlx::{PgPool, Result};
use uuid::Uuid;

// `query` rather than `query!` so the crate builds without a live database.

pub struct TransactionDao<'a> {
    pool: &'a PgPool,
}

impl<'a> TransactionDao<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert_tx(
        &self,
        id: Uuid,
        kind: &str,
        paths: &[String],
        hash_before: Option<&str>,
        hash_after: Option<&str>,
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO transactions (id, ts, kind, paths, hash_before, hash_after) \
             VALUES ($1, $2, $3, $4, $5, $6)",
        )
        .bind(id)
        .bind(Utc::now())
        .bind(kind)
        .bind(paths)
        .bind(hash_before)
        .bind(hash_after)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_tx_kind(&self, id: Uuid) -> Result<Option<String>> {
        let row: Option<(String,)> = sqlx::query_as("SELECT kind FROM transactions WHERE id = $1")
            .bind(id)
            .fetch_optional(self.pool)
            .await?;
        Ok(row.map(|(kind,)| kind))
    }
}
