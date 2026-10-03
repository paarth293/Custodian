use sqlx::{PgPool, Result};
use uuid::Uuid;

/// Dimensionality of stored embeddings (must match `003_vectors.sql`).
pub const EMBEDDING_DIM: usize = 1536;

pub struct VectorDao<'a> {
    pool: &'a PgPool,
}

/// Formats an embedding as a pgvector literal, e.g. `[0.1,0.2]`.
fn to_pgvector(embedding: &[f32]) -> String {
    let items: Vec<String> = embedding.iter().map(f32::to_string).collect();
    format!("[{}]", items.join(","))
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
        embedding: &[f32; EMBEDDING_DIM],
    ) -> Result<()> {
        sqlx::query(
            "INSERT INTO document_chunks (id, path, hash, content, embedding) \
             VALUES ($1, $2, $3, $4, $5::vector)",
        )
        .bind(id)
        .bind(path)
        .bind(hash)
        .bind(content)
        .bind(to_pgvector(embedding))
        .execute(self.pool)
        .await?;
        Ok(())
    }

    /// Returns `(path, content, cosine_distance)` for the closest chunks whose
    /// distance is below `threshold`.
    pub async fn search_chunks(
        &self,
        query_embedding: &[f32; EMBEDDING_DIM],
        limit: i64,
        threshold: f32,
    ) -> Result<Vec<(String, String, f64)>> {
        sqlx::query_as(
            "SELECT path, content, (embedding <=> $1::vector) AS distance \
             FROM document_chunks \
             WHERE (embedding <=> $1::vector) < $2 \
             ORDER BY distance ASC \
             LIMIT $3",
        )
        .bind(to_pgvector(query_embedding))
        .bind(f64::from(threshold))
        .bind(limit)
        .fetch_all(self.pool)
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn formats_pgvector_literal() {
        assert_eq!(to_pgvector(&[0.5, -1.0, 2.0]), "[0.5,-1,2]");
    }
}
