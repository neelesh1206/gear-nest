//! `CampSaver` — the reference clean-HTTP tier scraper (SPEC §7).
//!
//! A thin adapter: it owns the transport tier and `CampSaver`'s URL
//! conventions, and delegates all field extraction to the shared
//! [`crate::scrapers::jsonld`] parser. Parsing is exercised offline against
//! `tests/fixtures/campsaver_product.html` so CI never depends on the live site.

use anyhow::Result;
use async_trait::async_trait;
use tracing::warn;

use crate::models::{Category, PriceUpdate, RawProduct, RawReview};
use crate::scrapers::transport::{Tier, Transport};
use crate::scrapers::{jsonld, StoreCrawler};

const STORE_ID: &str = "campsaver";
const BASE_URL: &str = "https://www.campsaver.com";
/// Cap per category crawl so one run cannot fan out unbounded.
const MAX_PRODUCTS_PER_CATEGORY: usize = 60;
/// Reviews pagination safety cap for the `?reviews_page=N` walk. The caller's
/// `max` (typically the SPEC §13 cap of 500 reviews/product) is the real
/// bound; this is only here to stop a runaway loop if a page never returns
/// empty for some reason.
const MAX_REVIEW_PAGES: u32 = 50;

/// Full-sync seed categories. Slugs come from the live site's category URL
/// scheme (`/tents-shelters`, etc.). Kept small on purpose — five mainstream
/// outdoor categories cover the catalog overlap we resolve cross-store.
const CATEGORIES: &[(&str, &str)] = &[
    ("tents-shelters", "Tents & Shelters"),
    ("sleeping-bags", "Sleeping Bags"),
    ("backpacks", "Backpacks"),
    ("camp-kitchen", "Camp Kitchen"),
    ("mens-apparel", "Men's Apparel"),
];

pub struct CampSaverScraper {
    transport: Box<dyn Transport>,
}

impl CampSaverScraper {
    pub fn new() -> Result<Self> {
        Ok(Self {
            transport: Tier::CleanHttp.transport(STORE_ID)?,
        })
    }

    fn category_url(category: &Category) -> String {
        format!("{BASE_URL}/{}", category.slug.trim_matches('/'))
    }

    /// PR6 passes the listing's stored `store_url`; a bare slug is resolved
    /// against the base URL.
    fn product_url(store_product_id: &str) -> String {
        if store_product_id.starts_with("http") {
            store_product_id.to_string()
        } else {
            format!("{BASE_URL}/{}", store_product_id.trim_matches('/'))
        }
    }

    fn reviews_page_url(store_product_id: &str, page: u32) -> String {
        let base = Self::product_url(store_product_id);
        let sep = if base.contains('?') { '&' } else { '?' };
        format!("{base}{sep}reviews_page={page}")
    }
}

#[async_trait]
impl StoreCrawler for CampSaverScraper {
    fn store_id(&self) -> &str {
        STORE_ID
    }

    async fn crawl_products(&self, category: &Category) -> Result<Vec<RawProduct>> {
        let listing = self.transport.get(&Self::category_url(category)).await?;
        let urls = jsonld::parse_listing_urls(&listing, BASE_URL);
        let mut out = Vec::new();
        for url in urls.into_iter().take(MAX_PRODUCTS_PER_CATEGORY) {
            match self.transport.get(&url).await {
                Ok(html) => match jsonld::parse_product(&html, &url, STORE_ID) {
                    Ok(product) => out.push(product),
                    Err(e) => warn!(url, error = %e, "campsaver: product parse skipped"),
                },
                Err(e) => warn!(url, error = %e, "campsaver: product fetch skipped"),
            }
        }
        Ok(out)
    }

    async fn fetch_price(&self, store_product_id: &str) -> Result<PriceUpdate> {
        let url = Self::product_url(store_product_id);
        let html = self.transport.get(&url).await?;
        jsonld::parse_price(&html, STORE_ID, store_product_id)
    }

    async fn fetch_reviews(&self, store_product_id: &str, max: usize) -> Result<Vec<RawReview>> {
        let mut out: Vec<RawReview> = Vec::new();
        let mut seen = std::collections::HashSet::new();
        for page in 1..=MAX_REVIEW_PAGES {
            if out.len() >= max {
                break;
            }
            let url = Self::reviews_page_url(store_product_id, page);
            let html = match self.transport.get(&url).await {
                Ok(h) => h,
                Err(e) => {
                    warn!(url, error = %e, "campsaver: reviews fetch skipped");
                    break;
                }
            };
            let batch = jsonld::parse_reviews(&html, STORE_ID, store_product_id);
            let mut new_in_batch = 0;
            for r in batch {
                if out.len() >= max {
                    break;
                }
                if seen.insert(r.source_review_id.clone()) {
                    out.push(r);
                    new_in_batch += 1;
                }
            }
            if new_in_batch == 0 {
                break;
            }
        }
        Ok(out)
    }

    fn categories(&self) -> Vec<Category> {
        CATEGORIES
            .iter()
            .map(|(slug, label)| Category {
                slug: (*slug).to_string(),
                label: (*label).to_string(),
            })
            .collect()
    }
}
