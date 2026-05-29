//! `CampSaver` — the reference clean-HTTP tier scraper (SPEC §7).
//!
//! Product data is extracted from the page's schema.org JSON-LD (the most
//! stable target on e-commerce markup) rather than brittle DOM selectors. The
//! pure `parse_*` functions are exercised offline against a committed
//! `tests/fixtures/campsaver_product.html` so CI never depends on the live site.

use std::sync::LazyLock;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use regex::Regex;
use serde_json::Value;
use tracing::warn;

use crate::models::{Category, PriceUpdate, RawProduct};
use crate::scrapers::transport::{Tier, Transport};
use crate::scrapers::StoreCrawler;

const STORE_ID: &str = "campsaver";
const BASE_URL: &str = "https://www.campsaver.com";
/// Cap per category crawl so one run cannot fan out unbounded.
const MAX_PRODUCTS_PER_CATEGORY: usize = 60;

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
}

#[async_trait]
impl StoreCrawler for CampSaverScraper {
    fn store_id(&self) -> &str {
        STORE_ID
    }

    async fn crawl_products(&self, category: &Category) -> Result<Vec<RawProduct>> {
        let listing = self.transport.get(&Self::category_url(category)).await?;
        let urls = parse_listing_urls(&listing, BASE_URL);
        let mut out = Vec::new();
        for url in urls.into_iter().take(MAX_PRODUCTS_PER_CATEGORY) {
            match self.transport.get(&url).await {
                Ok(html) => match parse_product(&html, &url) {
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
        parse_price(&html, store_product_id)
    }
}

/// Extract a `RawProduct` from a product page's JSON-LD.
pub fn parse_product(html: &str, url: &str) -> Result<RawProduct> {
    let nodes = extract_ld_json(html);
    let product = nodes
        .iter()
        .find(|n| ld_type_matches(n, "Product"))
        .context("no Product JSON-LD on page")?;

    let title = product
        .get("name")
        .and_then(Value::as_str)
        .map(clean_text)
        .filter(|t| !t.is_empty())
        .with_context(|| format!("Product JSON-LD missing name ({url})"))?;

    let offer = product.get("offers").map(first_offer);

    Ok(RawProduct {
        store_id: STORE_ID.into(),
        store_product_id: product
            .get("sku")
            .and_then(Value::as_str)
            .map_or_else(|| slug_from_url(url), str::to_string),
        url: url.to_string(),
        title,
        brand: product.get("brand").and_then(brand_name),
        category_path: nodes
            .iter()
            .find(|n| ld_type_matches(n, "BreadcrumbList"))
            .map(breadcrumb_path)
            .unwrap_or_default(),
        description: product
            .get("description")
            .and_then(Value::as_str)
            .map(clean_text),
        features: product
            .get("additionalProperty")
            .map(feature_list)
            .unwrap_or_default(),
        specs: build_specs(product),
        primary_image: product.get("image").and_then(first_image),
        gtin: ["gtin13", "gtin14", "gtin12", "gtin8", "gtin", "mpn"]
            .iter()
            .find_map(|k| product.get(*k).and_then(Value::as_str))
            .map(str::to_string),
        price: offer.and_then(|o| o.get("price")).and_then(price_string),
        in_stock: offer
            .and_then(|o| o.get("availability"))
            .and_then(Value::as_str)
            .map(|s| s.contains("InStock")),
        store_rating: product
            .get("aggregateRating")
            .and_then(|a| a.get("ratingValue"))
            .and_then(json_f32),
        store_review_count: product
            .get("aggregateRating")
            .and_then(|a| a.get("reviewCount").or_else(|| a.get("ratingCount")))
            .and_then(json_i32)
            .unwrap_or(0),
        raw_payload: product.clone(),
    })
}

/// Extract just the live price + stock for a known product.
pub fn parse_price(html: &str, store_product_id: &str) -> Result<PriceUpdate> {
    let nodes = extract_ld_json(html);
    let offer = nodes
        .iter()
        .find(|n| ld_type_matches(n, "Product"))
        .and_then(|p| p.get("offers"))
        .map(first_offer)
        .context("no Product offer JSON-LD on page")?;
    Ok(PriceUpdate {
        store_id: STORE_ID.into(),
        store_product_id: store_product_id.to_string(),
        price: offer.get("price").and_then(price_string),
        in_stock: offer
            .get("availability")
            .and_then(Value::as_str)
            .map(|s| s.contains("InStock")),
        fetched_at: Utc::now(),
    })
}

/// Product-page URLs from a category listing page: prefer a structured
/// `ItemList`, fall back to product-anchor hrefs.
pub fn parse_listing_urls(html: &str, base_url: &str) -> Vec<String> {
    let nodes = extract_ld_json(html);
    if let Some(items) = nodes
        .iter()
        .find(|n| ld_type_matches(n, "ItemList"))
        .and_then(|n| n.get("itemListElement"))
        .and_then(Value::as_array)
    {
        let urls: Vec<String> = items
            .iter()
            .filter_map(|el| {
                el.get("url")
                    .and_then(Value::as_str)
                    .or_else(|| el.get("item").and_then(Value::as_str))
                    .map(str::to_string)
            })
            .collect();
        if !urls.is_empty() {
            return urls;
        }
    }
    ANCHOR_RE
        .captures_iter(html)
        .map(|c| c[1].to_string())
        .filter(|href| is_product_href(href))
        .map(|href| absolutize(&href, base_url))
        .collect()
}

/// Heuristic for the anchor fallback: a `.html` page or a `/product` path.
fn is_product_href(href: &str) -> bool {
    let path = href.split(['?', '#']).next().unwrap_or(href);
    let is_html = path
        .rsplit('.')
        .next()
        .is_some_and(|ext| ext.eq_ignore_ascii_case("html"));
    is_html || href.contains("/product")
}

static LD_JSON_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?is)<script[^>]*type=["']application/ld\+json["'][^>]*>(.*?)</script>"#)
        .expect("ld+json regex compiles")
});

static ANCHOR_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?i)<a[^>]+href=["']([^"']+)["']"#).expect("anchor regex compiles")
});

fn extract_ld_json(html: &str) -> Vec<Value> {
    let mut out = Vec::new();
    for cap in LD_JSON_RE.captures_iter(html) {
        if let Ok(val) = serde_json::from_str::<Value>(cap[1].trim()) {
            flatten_ld(val, &mut out);
        }
    }
    out
}

/// Hoist `@graph` members and array entries into a flat node list.
fn flatten_ld(val: Value, out: &mut Vec<Value>) {
    match val {
        Value::Array(items) => {
            for item in items {
                flatten_ld(item, out);
            }
        }
        Value::Object(mut map) => {
            if let Some(graph) = map.remove("@graph") {
                flatten_ld(graph, out);
            }
            out.push(Value::Object(map));
        }
        _ => {}
    }
}

fn ld_type_matches(node: &Value, wanted: &str) -> bool {
    match node.get("@type") {
        Some(Value::String(s)) => s == wanted,
        Some(Value::Array(arr)) => arr.iter().any(|t| t.as_str() == Some(wanted)),
        _ => false,
    }
}

/// schema.org `offers` may be a single Offer or a list; take the first.
fn first_offer(offers: &Value) -> &Value {
    match offers {
        Value::Array(arr) => arr.first().unwrap_or(offers),
        _ => offers,
    }
}

fn brand_name(brand: &Value) -> Option<String> {
    brand.as_str().map(str::to_string).or_else(|| {
        brand
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string)
    })
}

fn first_image(image: &Value) -> Option<String> {
    match image {
        Value::String(s) => Some(s.clone()),
        Value::Array(arr) => arr.first().and_then(first_image),
        Value::Object(_) => image.get("url").and_then(Value::as_str).map(str::to_string),
        _ => None,
    }
}

fn breadcrumb_path(node: &Value) -> Vec<String> {
    let Some(items) = node.get("itemListElement").and_then(Value::as_array) else {
        return Vec::new();
    };
    items
        .iter()
        .filter_map(|el| {
            el.get("name")
                .and_then(Value::as_str)
                .or_else(|| {
                    el.get("item")
                        .and_then(|i| i.get("name"))
                        .and_then(Value::as_str)
                })
                .map(str::to_string)
        })
        .filter(|name| !name.eq_ignore_ascii_case("home"))
        .collect()
}

fn feature_list(props: &Value) -> Vec<String> {
    let Some(arr) = props.as_array() else {
        return Vec::new();
    };
    arr.iter()
        .filter_map(|p| {
            let value = p.get("value").and_then(Value::as_str)?;
            match p.get("name").and_then(Value::as_str) {
                Some(name) => Some(format!("{name}: {value}")),
                None => Some(value.to_string()),
            }
        })
        .collect()
}

fn build_specs(product: &Value) -> Value {
    let mut map = serde_json::Map::new();
    if let Some(arr) = product.get("additionalProperty").and_then(Value::as_array) {
        for p in arr {
            if let (Some(name), Some(value)) = (
                p.get("name").and_then(Value::as_str),
                p.get("value").and_then(Value::as_str),
            ) {
                map.insert(name.to_string(), Value::String(value.to_string()));
            }
        }
    }
    Value::Object(map)
}

fn price_string(v: &Value) -> Option<String> {
    match v {
        Value::String(s) if !s.is_empty() => Some(s.clone()),
        Value::Number(n) => n.as_f64().map(|f| format!("{f:.2}")),
        _ => None,
    }
}

/// JSON numbers and numeric strings → `f32`. serde does the numeric conversion
/// so there is no lossy `as` cast in our code.
fn json_f32(v: &Value) -> Option<f32> {
    match v {
        Value::Number(_) => serde_json::from_value::<f32>(v.clone()).ok(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn json_i32(v: &Value) -> Option<i32> {
    match v {
        Value::Number(_) => serde_json::from_value::<i32>(v.clone()).ok(),
        Value::String(s) => s.parse().ok(),
        _ => None,
    }
}

fn slug_from_url(url: &str) -> String {
    url.split(['?', '#'])
        .next()
        .unwrap_or(url)
        .trim_end_matches('/')
        .rsplit('/')
        .next()
        .unwrap_or(url)
        .to_string()
}

fn clean_text(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

fn absolutize(href: &str, base_url: &str) -> String {
    if href.starts_with("http") {
        href.to_string()
    } else {
        format!(
            "{}/{}",
            base_url.trim_end_matches('/'),
            href.trim_start_matches('/')
        )
    }
}
