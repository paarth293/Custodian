use sqlx::{PgPool, Result};
use uuid::Uuid;

pub struct VectorDao<'a> {
    pool: &'a PgPool,
}

impl<'a> VectorDao<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    pub async fn insert_chunk(
        &self,
        id: Uuid,
        path: &str,
        hash: &str,
        content: &str,
        embedding: &[f32; 1536],
    ) -> Result<()> {
        let embedding_str = format!("{:?}", embedding); // pgvector accepts string representation
        
        sqlx::query(
            r#"
            INSERT INTO document_chunks (id, path, hash, content, embedding)
            VALUES ($1, $2, $3, $4, $5::vector)
            "#
        )
        .bind(id)
        .bind(path)
        .bind(hash)
        .bind(content)
        .bind(embedding_str)
        .execute(self.pool)
        .await?;
        Ok(())
    }

    pub async fn search_chunks(
        &self,
        query_embedding: &[f32; 1536],
        limit: i64,
        threshold: f32,
    ) -> Result<Vec<(String, String, f64)>> {
        let embedding_str = format!("{:?}", query_embedding);
        
        // Return path, content, and the distance
        let rows: Vec<(String, String, f64)> = sqlx::query_as(
            r#"
            SELECT path, content, (embedding <=> $1::vector) AS distance
            FROM document_chunks
            WHERE (embedding <=> $1::vector) < $2
            ORDER BY distance ASC
            LIMIT $3
            "#
        )
        .bind(embedding_str)
        .bind(threshold as f64)
        .bind(limit)
        .fetch_all(self.pool)
        .await?;

        Ok(rows)
    }
}
