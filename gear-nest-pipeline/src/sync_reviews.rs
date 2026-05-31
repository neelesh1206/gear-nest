//! One-shot review-sync job (SPEC §13).
//!
//! For every active `store_listing` (EXACT | HIGH | MEDIUM only — CANDIDATE
//! rows are excluded per ADR-007), fetch up to [`MAX_REVIEWS_PER_PRODUCT`]
//! reviews from the store and upsert them through
//! [`crate::reviews::upsert_batch`]. Per-store throttling reuses
//! [`crate::price_sync::rate_limiters`] so reviews and prices share one
//! courtesy budget. Stage-1 cross-store dedup runs as a separate pass
//! ([`crate::dedup_reviews`]) so a re-sync that just refreshes one store
//! is not blocked on it.
//!
//! Scheduled externally (Cloud Scheduler → one-shot Cloud Run Job) per
//! ADR-0022, wired in Phase 5.
//!
//! REI is the one store where `fetch_reviews` benefits from a URL hint
//! (skips a CJ lookup), so we pass `store_url` instead of
//! `store_product_id` for it. Every other store accepts the id form.

use std::collections::HashMap;
use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::postgres::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::models::RawReview;
use crate::price_sync;
use crate::reviews;
use crate::scrapers::StoreCrawler;

/// SPEC §13 cap: up to 500 reviews per product × store. Per-store impls
/// may cap themselves lower (Amazon/Bazaarvoice/Shopify-app sites only
/// surface the first page server-side); the value is an upper bound.
pub const MAX_REVIEWS_PER_PRODUCT: usize = 500;

const MAX_RETRIES: u32 = 3;

#[derive(Debug, Default, Clone, Copy)]
pub struct SyncReviewsReport {
    pub listings_processed: usize,
    pub reviews_upserted: u64,
    pub listings_skipped: usize,
    pub listings_failed: usize,
}

#[allow(clippy::struct_field_names)]
struct ActiveListing {
    product_id: Uuid,
    store_id: String,
    store_product_id: String,
    store_url: String,
}

/// Entry point: build the real crawler set and run.
pub async fn run(cfg: &Config, pool: &PgPool) -> Result<SyncReviewsReport> {
    let crawlers = price_sync::build_crawlers(cfg)?;
    run_with_crawlers(pool, &crawlers).await
}

/// The sync loop, with the crawler set injected so tests can pass a fake.
pub async fn run_with_crawlers<S: std::hash::BuildHasher>(
    pool: &PgPool,
    crawlers: &HashMap<&str, Box<dyn StoreCrawler>, S>,
) -> Result<SyncReviewsReport> {
    let listings = active_listings(pool).await?;
    info!(count = listings.len(), "sync-reviews: active listings");
    let limiters = price_sync::rate_limiters();

    let mut report = SyncReviewsReport::default();

    for listing in listings {
        report.listings_processed += 1;
        let Some(crawler) = crawlers.get(listing.store_id.as_str()) else {
            warn!(
                store = listing.store_id,
                "sync-reviews: no crawler for store"
            );
            report.listings_skipped += 1;
            continue;
        };
        if let Some(limiter) = limiters.get(listing.store_id.as_str()) {
            limiter.until_ready().await;
        }
        let id_arg = review_id_arg(&listing);
        match fetch_reviews_with_retry(
            crawler.as_ref(),
            id_arg,
            MAX_REVIEWS_PER_PRODUCT,
            MAX_RETRIES,
        )
        .await
        {
            Ok(raws) => {
                if raws.is_empty() {
                    report.listings_skipped += 1;
                    continue;
                }
                let written = reviews::upsert_batch(pool, listing.product_id, &raws).await?;
                report.reviews_upserted += written;
            }
            Err(e) => {
                warn!(
                    store = listing.store_id,
                    id = listing.store_product_id,
                    error = %e,
                    "sync-reviews: fetch failed"
                );
                report.listings_failed += 1;
            }
        }
    }

    info!(
        listings_processed = report.listings_processed,
        reviews_upserted = report.reviews_upserted,
        listings_skipped = report.listings_skipped,
        listings_failed = report.listings_failed,
        "sync-reviews complete"
    );
    Ok(report)
}

/// REI's `fetch_reviews` short-circuits a CJ lookup when handed a URL;
/// every other crawler treats `store_product_id` as the native id.
fn review_id_arg(listing: &ActiveListing) -> &str {
    if listing.store_id == "rei" {
        &listing.store_url
    } else {
        &listing.store_product_id
    }
}

async fn active_listings(pool: &PgPool) -> Result<Vec<ActiveListing>> {
    let rows: Vec<(Uuid, String, String, String)> = sqlx::query_as(
        "SELECT product_id, store_id, store_product_id, store_url FROM store_listings \
         WHERE match_confidence IN ('EXACT','HIGH','MEDIUM') ORDER BY store_id",
    )
    .fetch_all(pool)
    .await
    .context("querying active store_listings")?;
    Ok(rows
        .into_iter()
        .map(
            |(product_id, store_id, store_product_id, store_url)| ActiveListing {
                product_id,
                store_id,
                store_product_id,
                store_url,
            },
        )
        .collect())
}

async fn fetch_reviews_with_retry(
    crawler: &dyn StoreCrawler,
    store_product_id: &str,
    max: usize,
    max_retries: u32,
) -> Result<Vec<RawReview>> {
    let mut attempt = 0;
    loop {
        match crawler.fetch_reviews(store_product_id, max).await {
            Ok(rs) => return Ok(rs),
            Err(e) => {
                if attempt >= max_retries || !is_transient(&e) {
                    return Err(e);
                }
                tokio::time::sleep(Duration::from_millis(200 * 2u64.pow(attempt))).await;
                attempt += 1;
            }
        }
    }
}

/// Same heuristic as [`crate::price_sync`]: 429 / 503 / timeouts retry;
/// parse errors and 404s do not.
fn is_transient(e: &anyhow::Error) -> bool {
    let msg = e.to_string();
    msg.contains("429") || msg.contains("503") || msg.contains("timed out")
}
