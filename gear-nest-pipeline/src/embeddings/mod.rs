//! HuggingFace Inference API embedding client + pgvector bulk insert.
//!
//! Phase 1 wires the data path; HNSW is intentionally absent (ADR-001), the
//! chunk tables have a plain btree on `product_id` and chat does exact KNN
//! over the ~75 rows per product.

use std::time::Duration;

use anyhow::{Context, Result};
use reqwest::Client;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sqlx::postgres::PgPool;
use tracing::{debug, info, warn};
use uuid::Uuid;

pub mod chunk;

/// Embedding model dimension. The schema (`vector(384)`) hard-codes this; if
/// you change the model upstream you also change the migration.
pub const EMBEDDING_DIM: usize = 384;

/// HF Inference API soft-limit per request. The endpoint will accept larger
/// arrays but latency jumps superlinearly past ~64 inputs.
const BATCH_SIZE: usize = 32;

pub struct HuggingFaceEmbedder {
    client: Client,
    token: Option<String>,
    model: String,
    base_url: String,
}

// HF retired api-inference.huggingface.co; embeddings route through the
// Inference Providers router (hf-inference provider).
const DEFAULT_HF_BASE_URL: &str = "https://router.huggingface.co";

impl HuggingFaceEmbedder {
    pub fn new(token: Option<String>, model: String) -> Result<Self> {
        Self::with_base_url(token, model, None)
    }

    pub fn with_base_url(
        token: Option<String>,
        model: String,
        base_url: Option<String>,
    ) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(60))
            .user_agent("gear-nest-pipeline/0.1")
            .build()
            .context("building reqwest client")?;
        Ok(Self {
            client,
            token,
            model,
            base_url: base_url.unwrap_or_else(|| DEFAULT_HF_BASE_URL.into()),
        })
    }

    /// Embed an arbitrary number of inputs by batching into BATCH_SIZE chunks.
    /// Returns vectors in the same order as the inputs.
    pub async fn embed(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        if inputs.is_empty() {
            return Ok(Vec::new());
        }
        let mut out = Vec::with_capacity(inputs.len());
        for chunk in inputs.chunks(BATCH_SIZE) {
            let resp = self.embed_chunk(chunk).await?;
            out.extend(resp);
        }
        Ok(out)
    }

    async fn embed_chunk(&self, inputs: &[String]) -> Result<Vec<Vec<f32>>> {
        let url = format!(
            "{}/hf-inference/models/{}/pipeline/feature-extraction",
            self.base_url.trim_end_matches('/'),
            self.model
        );
        let body = json!({ "inputs": inputs });

        let mut req = self.client.post(&url).json(&body);
        if let Some(t) = self.token.as_deref() {
            req = req.bearer_auth(t);
        }

        debug!(model = self.model.as_str(), count = inputs.len(), "HF embed");
        let resp = req.send().await.context("HF embed request failed")?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("HF embed {status}: {text}");
        }
        let raw: EmbeddingResponse = serde_json::from_str(&text).with_context(|| {
            let preview: String = text.chars().take(200).collect();
            format!("decoding HF embed response (first 200): {preview}")
        })?;
        let vectors = match raw {
            EmbeddingResponse::Batch(v) => v,
            EmbeddingResponse::Single(v) => vec![v],
        };
        for v in &vectors {
            if v.len() != EMBEDDING_DIM {
                anyhow::bail!(
                    "HF returned vector of dim {}, expected {}",
                    v.len(),
                    EMBEDDING_DIM
                );
            }
        }
        Ok(vectors)
    }
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
enum EmbeddingResponse {
    Batch(Vec<Vec<f32>>),
    Single(Vec<f32>),
}

#[derive(Debug, Serialize, Clone)]
pub struct ReviewChunkInsert {
    pub review_id: Uuid,
    pub product_id: Uuid,
    pub chunk_text: String,
    pub chunk_index: i16,
    pub rating: Option<i16>,
    pub store_id: Option<String>,
}

#[derive(Debug, Serialize, Clone)]
pub struct SpecChunkInsert {
    pub product_id: Uuid,
    pub chunk_text: String,
    pub chunk_index: i16,
    pub source_type: SpecSource,
}

#[derive(Debug, Serialize, Clone, Copy)]
pub enum SpecSource {
    Description,
    Specs,
    Features,
}

impl SpecSource {
    fn as_db_str(self) -> &'static str {
        match self {
            Self::Description => "description",
            Self::Specs => "specs",
            Self::Features => "features",
        }
    }
}

/// Bulk-insert review chunks with embeddings via a single multi-row INSERT.
/// pgvector accepts the textual `[f1,f2,...]` literal cast to `vector`.
pub async fn insert_review_chunks(
    pool: &PgPool,
    rows: &[ReviewChunkInsert],
    embeddings: &[Vec<f32>],
) -> Result<u64> {
    if rows.is_empty() {
        return Ok(0);
    }
    assert_eq!(rows.len(), embeddings.len(), "rows/embeddings length mismatch");

    let mut sql = String::from(
        "INSERT INTO review_chunks \
         (review_id, product_id, chunk_text, chunk_index, embedding, rating, store_id) VALUES ",
    );
    let mut binds: Vec<String> = Vec::with_capacity(rows.len());
    for i in 0..rows.len() {
        let b = i * 7;
        binds.push(format!(
            "(${},${},${},${},${}::vector,${},${})",
            b + 1,
            b + 2,
            b + 3,
            b + 4,
            b + 5,
            b + 6,
            b + 7
        ));
    }
    sql.push_str(&binds.join(","));

    let mut q = sqlx::query(&sql);
    for (row, emb) in rows.iter().zip(embeddings.iter()) {
        q = q
            .bind(row.review_id)
            .bind(row.product_id)
            .bind(&row.chunk_text)
            .bind(row.chunk_index)
            .bind(vector_literal(emb))
            .bind(row.rating)
            .bind(row.store_id.as_deref());
    }
    let result = q.execute(pool).await?;
    info!(inserted = result.rows_affected(), "review_chunks bulk insert");
    Ok(result.rows_affected())
}

/// Bulk-insert spec chunks with embeddings.
pub async fn insert_spec_chunks(
    pool: &PgPool,
    rows: &[SpecChunkInsert],
    embeddings: &[Vec<f32>],
) -> Result<u64> {
    if rows.is_empty() {
        return Ok(0);
    }
    assert_eq!(rows.len(), embeddings.len(), "rows/embeddings length mismatch");

    let mut sql = String::from(
        "INSERT INTO spec_chunks \
         (product_id, chunk_text, chunk_index, source_type, embedding) VALUES ",
    );
    let mut binds: Vec<String> = Vec::with_capacity(rows.len());
    for i in 0..rows.len() {
        let b = i * 5;
        binds.push(format!(
            "(${},${},${},${},${}::vector)",
            b + 1,
            b + 2,
            b + 3,
            b + 4,
            b + 5
        ));
    }
    sql.push_str(&binds.join(","));

    let mut q = sqlx::query(&sql);
    for (row, emb) in rows.iter().zip(embeddings.iter()) {
        q = q
            .bind(row.product_id)
            .bind(&row.chunk_text)
            .bind(row.chunk_index)
            .bind(row.source_type.as_db_str())
            .bind(vector_literal(emb));
    }
    let result = q.execute(pool).await?;
    info!(inserted = result.rows_affected(), "spec_chunks bulk insert");
    Ok(result.rows_affected())
}

/// pgvector's text input format: `[1.0,2.0,3.0]`.
fn vector_literal(v: &[f32]) -> String {
    let mut s = String::with_capacity(v.len() * 8);
    s.push('[');
    for (i, x) in v.iter().enumerate() {
        if i > 0 {
            s.push(',');
        }
        // Pgvector accepts any float repr; default precision is fine.
        s.push_str(&format!("{x}"));
    }
    s.push(']');
    s
}

/// Convenience: embed a normalized product's spec text and insert all spec
/// chunks for it in one shot. Returns the number of chunks written.
pub async fn embed_and_insert_product_specs(
    embedder: &HuggingFaceEmbedder,
    pool: &PgPool,
    product_id: Uuid,
    description: Option<&str>,
    features: &[String],
) -> Result<u64> {
    let mut rows: Vec<SpecChunkInsert> = Vec::new();
    let mut texts: Vec<String> = Vec::new();

    if let Some(desc) = description.filter(|d| !d.is_empty()) {
        for (idx, chunk_text) in chunk::sentence_chunks(desc).into_iter().enumerate() {
            rows.push(SpecChunkInsert {
                product_id,
                chunk_text: chunk_text.clone(),
                chunk_index: idx as i16,
                source_type: SpecSource::Description,
            });
            texts.push(chunk_text);
        }
    }
    for (idx, feat) in features.iter().enumerate() {
        rows.push(SpecChunkInsert {
            product_id,
            chunk_text: feat.clone(),
            chunk_index: (rows.len() + idx) as i16,
            source_type: SpecSource::Features,
        });
        texts.push(feat.clone());
    }
    if texts.is_empty() {
        warn!(%product_id, "no spec text to embed");
        return Ok(0);
    }
    let embeddings = embedder.embed(&texts).await?;
    insert_spec_chunks(pool, &rows, &embeddings).await
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vector_literal_format() {
        let v = vec![1.0_f32, -0.5, 0.25];
        assert_eq!(vector_literal(&v), "[1,-0.5,0.25]");
    }
}
