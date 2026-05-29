//! Garage Grown Gear — clean-HTTP tier scraper for a Shopify cottage-gear store.
//!
//! Same shape as [`crate::scrapers::campsaver`]: owns the transport tier and
//! Shopify's `/collections/{handle}` + `/products/{handle}` URL conventions,
//! and delegates field extraction to the shared [`crate::scrapers::jsonld`]
//! parser. Tested offline against `tests/fixtures/garagegrowngear_product.html`.

use anyhow::Result;
use async_trait::async_trait;
use tracing::warn;

use crate::models::{Category, PriceUpdate, RawProduct};
use crate::scrapers::transport::{Tier, Transport};
use crate::scrapers::{jsonld, StoreCrawler};

/// The `stores` seed (migration 0001) registers this store under the slug
/// `garagerowngear`; it must match for the `store_listings.store_id` FK.
const STORE_ID: &str = "garagerowngear";
const BASE_URL: &str = "https://www.garagegrowngear.com";
/// Small indie site — crawl gently (SPEC §7 assigns it the lowest rate limit).
const MAX_PRODUCTS_PER_CATEGORY: usize = 40;

pub struct GarageGrownGearScraper {
    transport: Box<dyn Transport>,
}

impl GarageGrownGearScraper {
    pub fn new() -> Result<Self> {
        Ok(Self {
            transport: Tier::CleanHttp.transport(STORE_ID)?,
        })
    }

    fn category_url(category: &Category) -> String {
        format!("{BASE_URL}/collections/{}", category.slug.trim_matches('/'))
    }

    /// PR6 passes the listing's stored `store_url`; a bare handle is resolved
    /// to the Shopify product path.
    fn product_url(store_product_id: &str) -> String {
        if store_product_id.starts_with("http") {
            store_product_id.to_string()
        } else {
            format!("{BASE_URL}/products/{}", store_product_id.trim_matches('/'))
        }
    }
}

#[async_trait]
impl StoreCrawler for GarageGrownGearScraper {
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
                    Err(e) => warn!(url, error = %e, "garagegrowngear: product parse skipped"),
                },
                Err(e) => warn!(url, error = %e, "garagegrowngear: product fetch skipped"),
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
