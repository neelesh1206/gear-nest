//! Per-store scrapers behind the [`StoreCrawler`] trait.
//!
//! Trait dispatch keeps transport details (PA API, raw HTTP, headless browser)
//! out of the normalizer and downstream pipeline. Adding a store is one file.

use anyhow::Result;
use async_trait::async_trait;
use sqlx::postgres::PgPool;
use tracing::info;
use uuid::Uuid;

use crate::models::RawProduct;

pub mod amazon;

#[async_trait]
pub trait StoreCrawler: Send + Sync {
    /// Stable store identifier matching the `stores.id` column.
    fn store_id(&self) -> &str;

    /// Fetch a batch of products by the store's native product identifier.
    /// The Amazon PA API accepts up to 10 ASINs per `GetItems` call; other
    /// crawlers will batch differently. Implementations chunk internally.
    async fn fetch_batch(&self, ids: &[String]) -> Result<Vec<RawProduct>>;
}

/// Persist a raw scrape payload to a per-run audit log. Idempotent on
/// `(store_id, store_product_id)` via the `store_listings` UNIQUE constraint.
///
/// We deliberately do not write to `store_listings` here because that requires
/// a resolved canonical `product_id` — that's the job of the
/// [`crate::entity_resolution`] tier.
pub async fn record_raw(pool: &PgPool, raw: &RawProduct) -> Result<Uuid> {
    ensure_scrape_audit(pool).await?;
    let id: Uuid = sqlx::query_scalar(
        r#"
        INSERT INTO _gn_scrape_audit (store_id, store_product_id, payload)
        VALUES ($1, $2, $3)
        RETURNING id
        "#,
    )
    .bind(&raw.store_id)
    .bind(&raw.store_product_id)
    .bind(&raw.raw_payload)
    .fetch_one(pool)
    .await?;
    info!(
        store = raw.store_id.as_str(),
        product = raw.store_product_id.as_str(),
        audit_id = %id,
        "raw scrape recorded"
    );
    Ok(id)
}

/// Create the scrape-audit log table if absent. This is a pipeline-internal
/// table (prefix `_gn_`) and is not part of the user-facing schema in
/// `supabase/migrations/`.
pub async fn ensure_scrape_audit(pool: &PgPool) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _gn_scrape_audit (
            id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            store_id            TEXT NOT NULL,
            store_product_id    TEXT NOT NULL,
            payload             JSONB NOT NULL,
            scraped_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await?;
    sqlx::query(
        "CREATE INDEX IF NOT EXISTS _gn_scrape_audit_lookup_idx \
         ON _gn_scrape_audit (store_id, store_product_id, scraped_at DESC)",
    )
    .execute(pool)
    .await?;
    Ok(())
}
