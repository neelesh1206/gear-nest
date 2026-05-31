//! Cabela's — headless-tier scraper (SPEC §7).
//!
//! Cabela's gates product data behind JavaScript and aggressive bot protection,
//! so it runs on the `Tier::Headless` transport (a `chromiumoxide` browser pool
//! that renders the page). The rendered DOM still carries schema.org JSON-LD, so
//! field extraction is the shared [`crate::scrapers::jsonld`] parser — only the
//! transport differs. Cabela's runs last in the daily pipeline (SPEC §7). See
//! ADR-015.

use anyhow::Result;
use async_trait::async_trait;
use tracing::warn;

use crate::models::{Category, PriceUpdate, RawProduct, RawReview};
use crate::scrapers::transport::{Tier, Transport};
use crate::scrapers::{jsonld, StoreCrawler};

const STORE_ID: &str = "cabelas";
const BASE_URL: &str = "https://www.cabelas.com";
/// Headless is the most expensive tier — crawl a small slice per category.
const MAX_PRODUCTS_PER_CATEGORY: usize = 30;

/// Full-sync seed categories. Slugs follow the live site's nested
/// `/camping/{leaf}` scheme (matches the captured fixture's breadcrumb).
const CATEGORIES: &[(&str, &str)] = &[
    ("camping/tents", "Tents"),
    ("camping/sleeping-bags", "Sleeping Bags"),
    ("camping/backpacks", "Backpacks"),
    ("camping/stoves", "Stoves"),
    ("camping/coolers", "Coolers"),
];

pub struct CabelasScraper {
    transport: Box<dyn Transport>,
}

impl CabelasScraper {
    pub fn new() -> Result<Self> {
        Ok(Self {
            transport: Tier::Headless.transport(STORE_ID)?,
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
}

#[async_trait]
impl StoreCrawler for CabelasScraper {
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
                    Err(e) => warn!(url, error = %e, "cabelas: product parse skipped"),
                },
                Err(e) => warn!(url, error = %e, "cabelas: product fetch skipped"),
            }
        }
        Ok(out)
    }

    async fn fetch_price(&self, store_product_id: &str) -> Result<PriceUpdate> {
        let url = Self::product_url(store_product_id);
        let html = self.transport.get(&url).await?;
        jsonld::parse_price(&html, STORE_ID, store_product_id)
    }

    /// Cabela's review section is a Bazaarvoice widget rendered after page
    /// load. The headless transport waits for render and snapshots the DOM,
    /// so post-render JSON-LD (Bazaarvoice injects `<script type="ld+json">`
    /// for the loaded review batch) feeds the shared parser. Pagination
    /// happens inside the widget; a single render captures the first page.
    /// Caller `max` caps the snapshot.
    async fn fetch_reviews(&self, store_product_id: &str, max: usize) -> Result<Vec<RawReview>> {
        let url = Self::product_url(store_product_id);
        let html = self.transport.get(&url).await?;
        let mut reviews = jsonld::parse_reviews(&html, STORE_ID, store_product_id);
        reviews.truncate(max);
        Ok(reviews)
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
