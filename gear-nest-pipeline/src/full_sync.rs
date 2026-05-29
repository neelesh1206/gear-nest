//! One-shot full crawl across every scrape store (SPEC §7, PHASE2 PR 6 part 2).
//!
//! Closes the crawl→persist seam that [`crate::price_sync`] depends on: for
//! every crawler with a non-empty [`StoreCrawler::categories`] list, walk each
//! category → `crawl_products` → drive each raw through the same pipeline
//! `scrape-amazon` runs (`record_raw` → normalize → resolve → embed specs →
//! upsert listing → Redis SWR + `price_history`). Without this, only the
//! Amazon ASIN-driven path created `store_listings`, so cross-store entity
//! resolution (ADR-007) never exercised real data.
//!
//! One-shot CLI per ADR-0022 — scheduling is external (Cloud Scheduler →
//! Cloud Run Job, `0 2 * * 0` weekly). Per-store failures are logged and
//! skipped so one bad store cannot abort the run.

use std::collections::HashMap;

use anyhow::Result;
use chrono::Utc;
use sqlx::postgres::PgPool;
use tracing::{info, warn};

use crate::config::Config;
use crate::embeddings::{embed_and_insert_product_specs, HuggingFaceEmbedder};
use crate::entity_resolution::Resolver;
use crate::models::{PriceRecord, RawProduct};
use crate::normalizer;
use crate::price_history;
use crate::price_sync::{self, rate_limiters};
use crate::prices::{PricePayload, PriceWriter};
use crate::scrapers::{record_raw, StoreCrawler};

#[derive(Debug, Default, Clone, Copy)]
pub struct FullSyncReport {
    /// Listings successfully resolved + persisted.
    pub products: usize,
    /// Raw rows that failed somewhere in the pipeline.
    pub failed: usize,
    /// Stores that errored out at the category level (entire store skipped).
    pub stores_skipped: usize,
}

/// Production entrypoint: build every crawler from [`Config`], then crawl.
pub async fn run(
    cfg: &Config,
    pool: &PgPool,
    writer: &mut PriceWriter,
    embedder: &HuggingFaceEmbedder,
) -> Result<FullSyncReport> {
    let crawlers = price_sync::build_crawlers(cfg)?;
    run_with_crawlers(pool, writer, embedder, &crawlers).await
}

/// Crawl loop with the crawler set injected so tests can pass fakes.
pub async fn run_with_crawlers<S: std::hash::BuildHasher>(
    pool: &PgPool,
    writer: &mut PriceWriter,
    embedder: &HuggingFaceEmbedder,
    crawlers: &HashMap<&str, Box<dyn StoreCrawler>, S>,
) -> Result<FullSyncReport> {
    let limiters = rate_limiters();
    let resolver = Resolver::new(pool);
    let mut report = FullSyncReport::default();

    let mut store_ids: Vec<&str> = crawlers.keys().copied().collect();
    store_ids.sort_unstable();

    for store_id in store_ids {
        let crawler = crawlers
            .get(store_id)
            .expect("store_id sourced from the same map");
        let categories = crawler.categories();
        if categories.is_empty() {
            info!(store = store_id, "full-sync: no categories — skipping");
            continue;
        }

        let mut store_errored = false;
        for category in &categories {
            if let Some(limiter) = limiters.get(store_id) {
                limiter.until_ready().await;
            }
            let raws = match crawler.crawl_products(category).await {
                Ok(r) => r,
                Err(e) => {
                    warn!(store = store_id, category = %category.slug, error = %e, "full-sync: crawl failed");
                    store_errored = true;
                    continue;
                }
            };
            info!(
                store = store_id,
                category = %category.slug,
                count = raws.len(),
                "full-sync: crawled"
            );
            for raw in raws {
                match ingest_one(pool, writer, embedder, &resolver, raw).await {
                    Ok(()) => report.products += 1,
                    Err(e) => {
                        warn!(store = store_id, error = %e, "full-sync: ingest failed");
                        report.failed += 1;
                    }
                }
            }
        }
        if store_errored {
            report.stores_skipped += 1;
        }
    }

    writer.set_last_updated(Utc::now()).await?;
    info!(
        products = report.products,
        failed = report.failed,
        stores_skipped = report.stores_skipped,
        "full-sync complete"
    );
    Ok(report)
}

/// One raw product → audit row → normalize → resolve → embed → upsert listing
/// → Redis SWR + `price_history`. Mirrors the `scrape-amazon` loop in
/// `main.rs` so a store sees the same pipeline regardless of how it was
/// discovered.
async fn ingest_one(
    pool: &PgPool,
    writer: &mut PriceWriter,
    embedder: &HuggingFaceEmbedder,
    resolver: &Resolver<'_>,
    raw: RawProduct,
) -> Result<()> {
    record_raw(pool, &raw).await?;
    let norm = normalizer::normalize(&raw);
    let resolution = resolver.resolve(&raw, &norm).await?;
    info!(
        store = raw.store_id.as_str(),
        product_id = %resolution.product_id,
        confidence = resolution.confidence.as_db_str(),
        created = resolution.created,
        "full-sync resolved"
    );

    let features: Vec<String> = norm
        .specs
        .get("features")
        .and_then(|v| v.as_array())
        .map(|arr| {
            arr.iter()
                .filter_map(|x| x.as_str().map(str::to_string))
                .collect()
        })
        .unwrap_or_default();
    if let Err(e) = embed_and_insert_product_specs(
        embedder,
        pool,
        resolution.product_id,
        norm.description.as_deref(),
        &features,
    )
    .await
    {
        warn!(error = %e, "spec embed failed (continuing)");
    }

    if let Some(price) = raw.price.as_deref() {
        let listing_id = listing_id_for(pool, &raw.store_id, &raw.store_product_id).await?;
        let now = Utc::now();
        writer
            .write(
                resolution.product_id,
                &raw.store_id,
                PricePayload {
                    listing_id,
                    price: price.to_string(),
                    in_stock: raw.in_stock,
                    fetched_at: now,
                    jitter_secs: 0,
                },
            )
            .await?;
        price_history::append(
            pool,
            &PriceRecord {
                listing_id,
                price: price.to_string(),
                in_stock: raw.in_stock,
                fetched_at: now,
            },
        )
        .await?;
    }
    Ok(())
}

async fn listing_id_for(
    pool: &PgPool,
    store_id: &str,
    store_product_id: &str,
) -> Result<uuid::Uuid> {
    let (id,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT id FROM store_listings WHERE store_id = $1 AND store_product_id = $2",
    )
    .bind(store_id)
    .bind(store_product_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}
