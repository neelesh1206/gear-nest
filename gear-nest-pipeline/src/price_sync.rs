//! Daily price-sync job (SPEC §7).
//!
//! For every active `store_listing` (EXACT | HIGH | MEDIUM confidence only —
//! CANDIDATE rows are excluded per ADR-007) it refreshes the live price + stock
//! from the store, writes the Redis stale-while-revalidate hash (ADR-009), and
//! appends to `price_history` (ADR-010). Each store is throttled by a per-store
//! `governor` rate limit (SPEC §7); transient 429/503/timeout errors retry with
//! exponential backoff.

use std::collections::HashMap;
use std::num::NonZeroU32;
use std::time::Duration;

use anyhow::{Context, Result};
use chrono::Utc;
use governor::{DefaultDirectRateLimiter, Quota, RateLimiter};
use sqlx::postgres::PgPool;
use tracing::{info, warn};
use uuid::Uuid;

use crate::config::Config;
use crate::models::{PriceRecord, PriceUpdate};
use crate::price_history;
use crate::prices::{PricePayload, PriceWriter};
use crate::scrapers::{
    amazon::AmazonScraper, backcountry::BackcountryScraper, cabelas::CabelasScraper,
    campsaver::CampSaverScraper, garagegrowngear::GarageGrownGearScraper,
    moosejaw::MoosejawScraper, rei::ReiScraper, steepandcheap::SteepAndCheapScraper, StoreCrawler,
};

/// Per-store courtesy rate limits in requests/sec (SPEC §7). Keys match
/// `stores.id`. Shared with [`crate::full_sync`] so both jobs throttle each
/// store on the same budget.
pub(crate) const STORE_RATE_LIMITS: &[(&str, u32)] = &[
    ("amazon", 1),
    ("rei", 3),
    ("backcountry", 2),
    ("cabelas", 2),
    ("moosejaw", 3),
    ("steepandcheap", 3),
    ("campsaver", 2),
    ("garagerowngear", 1),
];

const MAX_RETRIES: u32 = 3;

#[derive(Debug, Default, Clone, Copy)]
pub struct PriceSyncReport {
    pub synced: usize,
    pub skipped: usize,
    pub failed: usize,
}

// Fields mirror the `store_listings` columns one-to-one; the shared `_id`
// suffix is the schema's, not noise.
#[allow(clippy::struct_field_names)]
struct ActiveListing {
    listing_id: Uuid,
    product_id: Uuid,
    store_id: String,
    store_product_id: String,
}

/// Build every store crawler, keyed by `stores.id`.
pub fn build_crawlers(cfg: &Config) -> Result<HashMap<&'static str, Box<dyn StoreCrawler>>> {
    let mut m: HashMap<&'static str, Box<dyn StoreCrawler>> = HashMap::new();
    m.insert("amazon", Box::new(AmazonScraper::new(cfg.paapi.clone())?));
    m.insert("rei", Box::new(ReiScraper::new(cfg.cj.clone())?));
    m.insert("campsaver", Box::new(CampSaverScraper::new()?));
    m.insert("garagerowngear", Box::new(GarageGrownGearScraper::new()?));
    m.insert("backcountry", Box::new(BackcountryScraper::new()?));
    m.insert("moosejaw", Box::new(MoosejawScraper::new()?));
    m.insert("steepandcheap", Box::new(SteepAndCheapScraper::new()?));
    m.insert("cabelas", Box::new(CabelasScraper::new()?));
    Ok(m)
}

pub(crate) fn rate_limiters() -> HashMap<&'static str, DefaultDirectRateLimiter> {
    STORE_RATE_LIMITS
        .iter()
        .map(|(id, rps)| {
            let quota = Quota::per_second(NonZeroU32::new(*rps).expect("rate limit is non-zero"));
            (*id, RateLimiter::direct(quota))
        })
        .collect()
}

/// Daily entry point: refresh prices for all active listings across all stores.
pub async fn run(cfg: &Config, pool: &PgPool, writer: &mut PriceWriter) -> Result<PriceSyncReport> {
    let crawlers = build_crawlers(cfg)?;
    run_with_crawlers(pool, writer, &crawlers).await
}

/// The sync loop, with the crawler set injected so tests can pass a fake store.
pub async fn run_with_crawlers<S: std::hash::BuildHasher>(
    pool: &PgPool,
    writer: &mut PriceWriter,
    crawlers: &HashMap<&str, Box<dyn StoreCrawler>, S>,
) -> Result<PriceSyncReport> {
    let listings = active_listings(pool).await?;
    info!(count = listings.len(), "price-sync: active listings");
    let limiters = rate_limiters();

    let mut report = PriceSyncReport::default();
    let mut records: Vec<PriceRecord> = Vec::new();
    let now = Utc::now();

    for listing in listings {
        let Some(crawler) = crawlers.get(listing.store_id.as_str()) else {
            warn!(store = listing.store_id, "price-sync: no crawler for store");
            report.skipped += 1;
            continue;
        };
        if let Some(limiter) = limiters.get(listing.store_id.as_str()) {
            limiter.until_ready().await;
        }
        match fetch_price_with_retry(crawler.as_ref(), &listing.store_product_id, MAX_RETRIES).await
        {
            Ok(update) => {
                let Some(price) = update.price else {
                    warn!(
                        store = listing.store_id,
                        id = listing.store_product_id,
                        "price-sync: no price returned"
                    );
                    report.skipped += 1;
                    continue;
                };
                writer
                    .write(
                        listing.product_id,
                        &listing.store_id,
                        PricePayload {
                            listing_id: listing.listing_id,
                            price: price.clone(),
                            in_stock: update.in_stock,
                            fetched_at: now,
                            jitter_secs: 0,
                        },
                    )
                    .await?;
                records.push(PriceRecord {
                    listing_id: listing.listing_id,
                    price,
                    in_stock: update.in_stock,
                    fetched_at: now,
                });
                report.synced += 1;
            }
            Err(e) => {
                warn!(store = listing.store_id, id = listing.store_product_id, error = %e, "price-sync: fetch failed");
                report.failed += 1;
            }
        }
    }

    price_history::append_many(pool, &records).await?;
    writer.set_last_updated(now).await?;
    info!(
        synced = report.synced,
        skipped = report.skipped,
        failed = report.failed,
        "price-sync complete"
    );
    Ok(report)
}

async fn active_listings(pool: &PgPool) -> Result<Vec<ActiveListing>> {
    let rows: Vec<(Uuid, Uuid, String, String)> = sqlx::query_as(
        "SELECT id, product_id, store_id, store_product_id FROM store_listings \
         WHERE match_confidence IN ('EXACT','HIGH','MEDIUM') ORDER BY store_id",
    )
    .fetch_all(pool)
    .await
    .context("querying active store_listings")?;
    Ok(rows
        .into_iter()
        .map(
            |(listing_id, product_id, store_id, store_product_id)| ActiveListing {
                listing_id,
                product_id,
                store_id,
                store_product_id,
            },
        )
        .collect())
}

async fn fetch_price_with_retry(
    crawler: &dyn StoreCrawler,
    store_product_id: &str,
    max_retries: u32,
) -> Result<PriceUpdate> {
    let mut attempt = 0;
    loop {
        match crawler.fetch_price(store_product_id).await {
            Ok(update) => return Ok(update),
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

/// 429 / 503 / timeouts are worth retrying; parse errors and 404s are not.
fn is_transient(e: &anyhow::Error) -> bool {
    let msg = e.to_string();
    msg.contains("429") || msg.contains("503") || msg.contains("timed out")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rate_limits_cover_every_seeded_store() {
        // Every store the registry can build must have a courtesy rate limit.
        let stores = [
            "amazon",
            "rei",
            "campsaver",
            "garagerowngear",
            "backcountry",
            "moosejaw",
            "steepandcheap",
            "cabelas",
        ];
        for store in stores {
            assert!(
                STORE_RATE_LIMITS.iter().any(|(id, _)| *id == store),
                "missing rate limit for {store}"
            );
        }
        assert_eq!(STORE_RATE_LIMITS.len(), stores.len());
    }

    #[test]
    fn transient_errors_are_classified() {
        assert!(is_transient(&anyhow::anyhow!(
            "GET https://x -> HTTP 429 Too Many Requests"
        )));
        assert!(is_transient(&anyhow::anyhow!("GET https://x -> HTTP 503")));
        assert!(!is_transient(&anyhow::anyhow!(
            "no Product JSON-LD on page"
        )));
        assert!(!is_transient(&anyhow::anyhow!(
            "GET https://x -> HTTP 404 Not Found"
        )));
    }
}
