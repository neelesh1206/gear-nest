//! Moosejaw — proxy-tier scraper (SPEC §7).
//!
//! Salesforce Commerce Cloud store behind bot protection; runs on the
//! `Tier::Proxy` transport (residential proxy via `SCRAPE_PROXY_MOOSEJAW`) and
//! delegates field extraction to the shared [`crate::scrapers::jsonld`] parser.
//! See ADR-014.

use anyhow::Result;
use async_trait::async_trait;
use tracing::warn;

use crate::models::{Category, PriceUpdate, RawProduct};
use crate::scrapers::transport::{Tier, Transport};
use crate::scrapers::{jsonld, StoreCrawler};

const STORE_ID: &str = "moosejaw";
const BASE_URL: &str = "https://www.moosejaw.com";
const MAX_PRODUCTS_PER_CATEGORY: usize = 60;

pub struct MoosejawScraper {
    transport: Box<dyn Transport>,
}

impl MoosejawScraper {
    pub fn new() -> Result<Self> {
        Ok(Self {
            transport: Tier::Proxy.transport(STORE_ID)?,
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
impl StoreCrawler for MoosejawScraper {
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
                    Err(e) => warn!(url, error = %e, "moosejaw: product parse skipped"),
                },
                Err(e) => warn!(url, error = %e, "moosejaw: product fetch skipped"),
            }
        }
        Ok(out)
    }

    async fn fetch_price(&self, store_product_id: &str) -> Result<PriceUpdate> {
        let url = Self::product_url(store_product_id);
        let html = self.transport.get(&url).await?;
        jsonld::parse_price(&html, STORE_ID, store_product_id)
    }
}
