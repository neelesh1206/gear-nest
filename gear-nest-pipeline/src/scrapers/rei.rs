//! REI — API + scrape tier (SPEC §7).
//!
//! REI's catalog comes from the CJ (Commission Junction) affiliate GraphQL
//! Product Search API, which returns price, GTIN, image, and URL. CJ omits
//! structured specs / long descriptions / reviews, so each CJ record is
//! supplemented by a clean-HTTP scrape of the REI product page (the shared
//! [`crate::scrapers::jsonld`] parser) and merged: CJ is authoritative for
//! commerce fields, the scrape only fills blanks (see [`merge_supplement`]).

use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use reqwest::Client;
use serde_json::{json, Value};
use tracing::warn;

use crate::config::CjConfig;
use crate::models::{Category, PriceUpdate, RawProduct};
use crate::scrapers::transport::{Tier, Transport};
use crate::scrapers::{jsonld, StoreCrawler};

const STORE_ID: &str = "rei";

/// Full-sync seed categories. REI's discovery is keyword search against the
/// CJ affiliate API — `Category::label` becomes the search term — so these
/// are product keywords rather than URL slugs.
const CATEGORIES: &[(&str, &str)] = &[
    ("tent", "tent"),
    ("sleeping bag", "sleeping bag"),
    ("backpack", "backpack"),
    ("camp stove", "camp stove"),
    ("rain jacket", "rain jacket"),
];

const CJ_QUERY: &str =
    "query Products($companyId: ID!, $partnerIds: [ID!], $keywords: [String!], $limit: Int) { \
products(companyId: $companyId, partnerIds: $partnerIds, keywords: $keywords, limit: $limit) { \
resultList { id title description brand gtin imageLink link availability productType \
price { amount currency } salePrice { amount currency } } } }";

/// Client for CJ's affiliate GraphQL Product Search API.
pub struct CjClient {
    client: Client,
    config: CjConfig,
}

impl CjClient {
    pub fn new(config: CjConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("gear-nest-pipeline/0.1 (+https://gearnest.io)")
            .build()
            .context("building CJ client")?;
        Ok(Self { client, config })
    }

    fn require_creds(&self) -> Result<(&str, &str, &str)> {
        let company = self
            .config
            .website_id
            .as_deref()
            .context("CJ_WEBSITE_ID not set")?;
        let partner = self
            .config
            .advertiser_id
            .as_deref()
            .context("CJ_REI_ADVERTISER_ID not set")?;
        let key = self
            .config
            .api_key
            .as_deref()
            .context("CJ_API_KEY not set")?;
        Ok((company, partner, key))
    }

    /// Query CJ for REI products matching `keywords`.
    pub async fn search(&self, keywords: &str) -> Result<Vec<RawProduct>> {
        let (company, partner, key) = self.require_creds()?;
        let body = json!({
            "query": CJ_QUERY,
            "variables": {
                "companyId": company,
                "partnerIds": [partner],
                "keywords": [keywords],
                "limit": 50
            }
        });
        let resp = self
            .client
            .post(&self.config.endpoint)
            .bearer_auth(key)
            .json(&body)
            .send()
            .await
            .context("CJ product search request")?;
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("CJ search HTTP {status}: {text}");
        }
        parse_cj_response(&text)
    }
}

pub struct ReiScraper {
    cj: CjClient,
    scrape: Box<dyn Transport>,
}

impl ReiScraper {
    pub fn new(config: CjConfig) -> Result<Self> {
        Ok(Self {
            cj: CjClient::new(config)?,
            scrape: Tier::CleanHttp.transport(STORE_ID)?,
        })
    }
}

#[async_trait]
impl StoreCrawler for ReiScraper {
    fn store_id(&self) -> &str {
        STORE_ID
    }

    async fn crawl_products(&self, category: &Category) -> Result<Vec<RawProduct>> {
        let primaries = self.cj.search(&category.label).await?;
        let mut out = Vec::with_capacity(primaries.len());
        for primary in primaries {
            let fetched = self.scrape.get(&primary.url).await;
            let merged = match fetched {
                Ok(html) => match jsonld::parse_product(&html, &primary.url, STORE_ID) {
                    Ok(supplement) => merge_supplement(primary, supplement),
                    Err(e) => {
                        warn!(url = %primary.url, error = %e, "rei: supplement parse skipped");
                        primary
                    }
                },
                Err(e) => {
                    warn!(url = %primary.url, error = %e, "rei: supplement fetch skipped");
                    primary
                }
            };
            out.push(merged);
        }
        Ok(out)
    }

    async fn fetch_price(&self, store_product_id: &str) -> Result<PriceUpdate> {
        let item = self
            .cj
            .search(store_product_id)
            .await?
            .into_iter()
            .find(|p| p.store_product_id == store_product_id)
            .context("CJ returned no product for the requested id")?;
        Ok(PriceUpdate {
            store_id: STORE_ID.to_string(),
            store_product_id: store_product_id.to_string(),
            price: item.price,
            in_stock: item.in_stock,
            fetched_at: Utc::now(),
        })
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

/// Parse a CJ GraphQL response body into `RawProduct`s.
pub fn parse_cj_response(json: &str) -> Result<Vec<RawProduct>> {
    let root: Value = serde_json::from_str(json).context("decoding CJ response")?;
    if let Some(errors) = root.get("errors").and_then(Value::as_array) {
        if !errors.is_empty() {
            anyhow::bail!("CJ GraphQL errors: {errors:?}");
        }
    }
    let list = root
        .pointer("/data/products/resultList")
        .and_then(Value::as_array)
        .context("CJ response missing data.products.resultList")?;
    Ok(list.iter().filter_map(cj_item_to_raw).collect())
}

/// Merge a scraped product onto the CJ record. CJ wins on the commerce fields
/// (price, stock, GTIN, id, url, title); the scrape only fills what CJ left
/// blank (specs, description, category, image, rating). CJ is paid-on and must
/// match the click destination, so it stays authoritative for price + link.
pub fn merge_supplement(primary: RawProduct, supplement: RawProduct) -> RawProduct {
    RawProduct {
        store_id: primary.store_id,
        store_product_id: primary.store_product_id,
        url: primary.url,
        title: primary.title,
        brand: primary.brand.or(supplement.brand),
        category_path: if primary.category_path.is_empty() {
            supplement.category_path
        } else {
            primary.category_path
        },
        description: primary.description.or(supplement.description),
        features: if primary.features.is_empty() {
            supplement.features
        } else {
            primary.features
        },
        specs: if specs_is_empty(&primary.specs) {
            supplement.specs
        } else {
            primary.specs
        },
        primary_image: primary.primary_image.or(supplement.primary_image),
        // CJ-only: GTIN drives Tier-1 entity resolution (ADR-007), so a scraped
        // GTIN must never stand in for CJ's — even when CJ omits one (ADR-0023).
        gtin: primary.gtin,
        price: primary.price,
        in_stock: primary.in_stock,
        store_rating: primary.store_rating.or(supplement.store_rating),
        store_review_count: if primary.store_review_count == 0 {
            supplement.store_review_count
        } else {
            primary.store_review_count
        },
        raw_payload: primary.raw_payload,
    }
}

fn cj_item_to_raw(item: &Value) -> Option<RawProduct> {
    let store_product_id = item
        .get("id")
        .and_then(Value::as_str)
        .filter(|s| !s.is_empty())?
        .to_string();
    let title = item
        .get("title")
        .and_then(Value::as_str)
        .map(collapse_ws)
        .filter(|t| !t.is_empty())?;
    Some(RawProduct {
        store_id: STORE_ID.to_string(),
        store_product_id,
        url: item
            .get("link")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        title,
        brand: item
            .get("brand")
            .and_then(Value::as_str)
            .map(str::to_string),
        category_path: category_path(item),
        description: item
            .get("description")
            .and_then(Value::as_str)
            .map(collapse_ws),
        features: Vec::new(),
        specs: Value::Object(serde_json::Map::new()),
        primary_image: item
            .get("imageLink")
            .and_then(Value::as_str)
            .map(str::to_string),
        gtin: item
            .get("gtin")
            .and_then(Value::as_str)
            .filter(|s| !s.is_empty())
            .map(str::to_string),
        price: cj_price(item),
        in_stock: cj_in_stock(item),
        store_rating: None,
        store_review_count: 0,
        raw_payload: item.clone(),
    })
}

/// CJ gives a `salePrice` when on promotion; prefer it, else the list `price`.
fn cj_price(item: &Value) -> Option<String> {
    item.get("salePrice")
        .and_then(|p| p.get("amount"))
        .and_then(amount_string)
        .or_else(|| {
            item.get("price")
                .and_then(|p| p.get("amount"))
                .and_then(amount_string)
        })
}

/// CJ availability strings are lowercase prose ("in stock" / "out of stock").
/// Mirror `jsonld::availability_in_stock`: anything not explicitly unavailable
/// counts as in stock.
fn cj_in_stock(item: &Value) -> Option<bool> {
    item.get("availability").and_then(Value::as_str).map(|a| {
        let a = a.to_lowercase();
        !["out of stock", "sold out", "discontinued"]
            .iter()
            .any(|unavailable| a.contains(unavailable))
    })
}

fn category_path(item: &Value) -> Vec<String> {
    item.get("productType")
        .and_then(Value::as_str)
        .map(|s| {
            s.split('>')
                .map(|p| p.trim().to_string())
                .filter(|p| !p.is_empty())
                .collect()
        })
        .unwrap_or_default()
}

fn amount_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => n.as_f64().map(|f| format!("{f:.2}")),
        _ => None,
    }
}

fn specs_is_empty(v: &Value) -> bool {
    match v {
        Value::Object(m) => m.is_empty(),
        Value::Null => true,
        _ => false,
    }
}

fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
