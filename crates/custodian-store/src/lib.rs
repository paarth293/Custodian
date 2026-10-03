//! PostgreSQL persistence for transactions, embeddings and telemetry.

pub mod dao;
pub mod vector_dao;

use sqlx::{postgres::PgPoolOptions, PgPool};

/// Connection pool that applies pending migrations on start-up.
pub struct DbPool {
    pool: PgPool,
}

impl DbPool {
    pub async fn new(url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new().max_connections(5).connect(url).await?;
        sqlx::migrate!("./migrations").run(&pool).await?;
        Ok(Self { pool })
    }

    pub fn pool(&self) -> &PgPool {
        &self.pool
    }
}
