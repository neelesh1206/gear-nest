//! Per-store scrapers behind the [`StoreCrawler`] trait.
//!
//! Trait dispatch keeps transport details (PA API, raw HTTP, headless browser)
//! out of the normalizer and downstream pipeline. Adding a store is one file.

use anyhow::Result;
use async_trait::async_trait;
use sqlx::postgres::PgPool;
use tracing::info;
use uuid::Uuid;

use crate::models::{Category, PriceUpdate, RawProduct};

pub mod amazon;
pub mod backcountry;
pub mod cabelas;
pub mod campsaver;
pub mod garagegrowngear;
pub mod jsonld;
pub mod moosejaw;
pub mod rei;
pub mod steepandcheap;
pub mod transport;

/// Transport details (PA-API, clean HTTP, proxy, headless) stay behind this
/// trait so the normalizer / resolver / embedder never learn how a store is
/// reached. A store implements only the methods its tier supports; the others
/// keep their defaults and surface a loud error if mis-dispatched. See ADR-013.
#[async_trait]
pub trait StoreCrawler: Send + Sync {
    /// Stable store identifier matching the `stores.id` column.
    fn store_id(&self) -> &str;

    /// API stores fetch by their native product identifier. The Amazon PA-API
    /// accepts up to 10 ASINs per `GetItems` call; other API crawlers batch
    /// differently. Implementations chunk internally.
    async fn fetch_batch(&self, _ids: &[String]) -> Result<Vec<RawProduct>> {
        anyhow::bail!(
            "{} does not support fetch_batch (not an API store)",
            self.store_id()
        )
    }

    /// Scrape stores have no ID list — they discover products by crawling a
    /// category's landing pages.
    async fn crawl_products(&self, _category: &Category) -> Result<Vec<RawProduct>> {
        anyhow::bail!(
            "{} does not support crawl_products (not a scrape store)",
            self.store_id()
        )
    }

    /// Curated category seeds the `full-sync` job iterates over. Crawl stores
    /// override with a small static list (5-ish slugs, matching their site's
    /// URL conventions); API-only stores leave the default empty so full-sync
    /// skips them. Dynamic discovery is a follow-up if these seeds need to
    /// expand.
    fn categories(&self) -> Vec<Category> {
        Vec::new()
    }

    /// Refresh the live price + stock for a single known product. Used by the
    /// daily price-sync across all tiers.
    async fn fetch_price(&self, _store_product_id: &str) -> Result<PriceUpdate> {
        anyhow::bail!("{} does not implement fetch_price", self.store_id())
    }
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
        r"
        INSERT INTO _gn_scrape_audit (store_id, store_product_id, payload)
        VALUES ($1, $2, $3)
        RETURNING id
        ",
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
        r"
        CREATE TABLE IF NOT EXISTS _gn_scrape_audit (
            id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
            store_id            TEXT NOT NULL,
            store_product_id    TEXT NOT NULL,
            payload             JSONB NOT NULL,
            scraped_at          TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        ",
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
