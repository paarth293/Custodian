pub mod vector_dao;
pub mod dao;
use sqlx::{postgres::PgPoolOptions, PgPool};

pub struct DbPool {
    pool: PgPool,
}

impl DbPool {
    pub async fn new(url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(5)
            .connect(url)
            .await?;
        
        sqlx::migrate!("./migrations")
            .run(&pool)
            .await?;
            
        Ok(Self { pool })
    }

    pub fn get_pool(&self) -> &PgPool {
        &self.pool
    }
}


