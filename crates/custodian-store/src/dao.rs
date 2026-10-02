use sqlx::{PgPool, Result};
use uuid::Uuid;
use chrono::Utc;

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
        let ts = Utc::now();
        // Fallback to query() instead of query!() to ensure CI compiles without a live Postgres instance.
        sqlx::query(
            r#"
            INSERT INTO transactions (id, ts, kind, paths, hash_before, hash_after)
            VALUES ($1, $2, $3, $4, $5, $6)
            "#
        )
        .bind(id)
        .bind(ts)
        .bind(kind)
        .bind(paths)
        .bind(hash_before)
        .bind(hash_after)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn get_tx_kind(&self, id: Uuid) -> Result<Option<String>> {
        // Fallback to query() for CI compilation without Postgres.
        let row: Option<(String,)> = sqlx::query_as(
            r#"SELECT kind FROM transactions WHERE id = $1"#
        )
        .bind(id)
        .fetch_optional(self.pool)
        .await?;
        Ok(row.map(|r| r.0))
    }
}
