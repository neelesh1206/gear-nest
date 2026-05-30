//! Shared schema.org JSON-LD parsing for the clean-HTTP tier scrapers.
//!
//! Outdoor retailers (`CampSaver`, Garage Grown Gear, ...) embed structured
//! `Product` / `BreadcrumbList` / `ItemList` JSON-LD, which is far more stable
//! than DOM selectors. These functions parse it into the pipeline's
//! store-agnostic [`RawProduct`] / [`PriceUpdate`], stamped with the caller's
//! `store_id`. Each store module owns only its transport + URL conventions.

use std::sync::LazyLock;

use anyhow::{Context, Result};
use chrono::{NaiveDate, Utc};
use regex::Regex;
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::models::{PriceUpdate, RawProduct, RawReview};

/// Extract a `RawProduct` from a product page's JSON-LD.
pub fn parse_product(html: &str, url: &str, store_id: &str) -> Result<RawProduct> {
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
        store_id: store_id.to_string(),
        store_product_id: offer
            .and_then(|o| o.get("sku"))
            .or_else(|| product.get("sku"))
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
        gtin: gtin(product).or_else(|| offer.and_then(gtin)),
        price: offer.and_then(|o| o.get("price")).and_then(price_string),
        in_stock: offer
            .and_then(|o| o.get("availability"))
            .and_then(Value::as_str)
            .map(availability_in_stock),
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
pub fn parse_price(html: &str, store_id: &str, store_product_id: &str) -> Result<PriceUpdate> {
    let nodes = extract_ld_json(html);
    let offer = nodes
        .iter()
        .find(|n| ld_type_matches(n, "Product"))
        .and_then(|p| p.get("offers"))
        .map(first_offer)
        .context("no Product offer JSON-LD on page")?;
    Ok(PriceUpdate {
        store_id: store_id.to_string(),
        store_product_id: store_product_id.to_string(),
        price: offer.get("price").and_then(price_string),
        in_stock: offer
            .get("availability")
            .and_then(Value::as_str)
            .map(availability_in_stock),
        fetched_at: Utc::now(),
    })
}

/// Extract reviews embedded in a product page's JSON-LD.
///
/// Reviews may live either as `Product.review[]` or as standalone `Review`
/// nodes in an `@graph`. Both are surfaced. The schema.org core carries
/// rating + body + author + date but not retailer extensions like
/// `verified_purchase` or `helpful_votes`; those default to `false` / `0`
/// and are filled in by per-store parsers when the markup exposes them.
///
/// `source_review_id` is always populated: prefer the page's `@id` /
/// `identifier`, else a stable SHA-256 of the review content so the
/// `UNIQUE(store_id, source_review_id)` constraint can dedupe re-imports.
pub fn parse_reviews(html: &str, store_id: &str, store_product_id: &str) -> Vec<RawReview> {
    let nodes = extract_ld_json(html);
    let mut out: Vec<RawReview> = Vec::new();
    for product in nodes.iter().filter(|n| ld_type_matches(n, "Product")) {
        if let Some(arr) = product.get("review").and_then(Value::as_array) {
            for r in arr {
                if let Some(rev) = parse_one_review(r, store_id, store_product_id) {
                    out.push(rev);
                }
            }
        }
    }
    for r in nodes.iter().filter(|n| ld_type_matches(n, "Review")) {
        if let Some(rev) = parse_one_review(r, store_id, store_product_id) {
            out.push(rev);
        }
    }
    dedup_reviews_by_source_id(out)
}

fn parse_one_review(node: &Value, store_id: &str, store_product_id: &str) -> Option<RawReview> {
    let body = node
        .get("reviewBody")
        .or_else(|| node.get("description"))
        .and_then(Value::as_str)
        .map(clean_text)
        .filter(|b| !b.is_empty())?;
    let rating = node
        .get("reviewRating")
        .and_then(|r| r.get("ratingValue"))
        .and_then(json_f32)
        .map(rating_bucket)?;
    let author_name = node
        .get("author")
        .and_then(author_display_name)
        .map(|s| clean_text(&s))
        .filter(|s| !s.is_empty() && !s.eq_ignore_ascii_case("anonymous"));
    // Hash the bare author name (no `store_id` prefix) so the same reviewer
    // across stores collapses to the same hash. That's the load-bearing
    // property of `reviews.reviewer_id_hash` for SPEC §13 Stage-1 cross-store
    // dedup (PR 7). False-positive risk is bounded by the dedup scope, which
    // is `(product_id, reviewer_id_hash)` — two "Sarah K."s reviewing the
    // *same product* at two stores is rare and a verified-purchase tiebreak
    // keeps the more credible row.
    let reviewer_id_hash = author_name.as_deref().map(|name| {
        let mut h = Sha256::new();
        h.update(name.to_lowercase().as_bytes());
        hex::encode(h.finalize())
    });
    let review_date = node
        .get("datePublished")
        .and_then(Value::as_str)
        .and_then(parse_review_date);
    let title = node
        .get("name")
        .or_else(|| node.get("headline"))
        .and_then(Value::as_str)
        .map(clean_text)
        .filter(|t| !t.is_empty());
    let source_review_id = native_review_id(node)
        .unwrap_or_else(|| content_hash_id(&body, author_name.as_deref(), review_date.as_ref()));
    Some(RawReview {
        store_id: store_id.to_string(),
        store_product_id: store_product_id.to_string(),
        source_review_id,
        reviewer_id_hash,
        rating,
        title,
        body,
        verified_purchase: false,
        helpful_votes: 0,
        review_date,
    })
}

/// Bucket a schema.org rating value into the integer 1..=5 stored in the
/// `reviews.rating` column. Half-stars round, out-of-range values clamp.
fn rating_bucket(f: f32) -> i16 {
    if f <= 1.5 {
        1
    } else if f <= 2.5 {
        2
    } else if f <= 3.5 {
        3
    } else if f <= 4.5 {
        4
    } else {
        5
    }
}

fn author_display_name(author: &Value) -> Option<String> {
    match author {
        Value::String(s) => Some(s.clone()),
        Value::Object(_) => author
            .get("name")
            .and_then(Value::as_str)
            .map(str::to_string),
        Value::Array(arr) => arr.first().and_then(author_display_name),
        _ => None,
    }
}

fn native_review_id(node: &Value) -> Option<String> {
    ["@id", "identifier", "url"]
        .iter()
        .find_map(|k| node.get(*k).and_then(Value::as_str))
        .map(str::to_string)
}

fn content_hash_id(body: &str, author: Option<&str>, date: Option<&NaiveDate>) -> String {
    let mut h = Sha256::new();
    h.update(body.as_bytes());
    h.update(b"|");
    h.update(author.unwrap_or("").as_bytes());
    h.update(b"|");
    h.update(date.map_or(String::new(), ToString::to_string).as_bytes());
    format!("sha256:{}", hex::encode(h.finalize()))
}

/// schema.org `datePublished` is ISO-8601. We accept either a bare date
/// (`2025-03-15`) or a full timestamp (`2025-03-15T12:34:56Z`) and store
/// only the date.
fn parse_review_date(s: &str) -> Option<NaiveDate> {
    NaiveDate::parse_from_str(s, "%Y-%m-%d").ok().or_else(|| {
        s.split('T')
            .next()
            .and_then(|d| NaiveDate::parse_from_str(d, "%Y-%m-%d").ok())
    })
}

fn dedup_reviews_by_source_id(reviews: Vec<RawReview>) -> Vec<RawReview> {
    let mut seen = std::collections::HashSet::new();
    reviews
        .into_iter()
        .filter(|r| seen.insert(r.source_review_id.clone()))
        .collect()
}

/// Product-page URLs from a category listing page: prefer a structured
/// `ItemList`, fall back to product-anchor hrefs. Order-preserving + deduped.
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
            return dedup_in_order(urls);
        }
    }
    let anchors = ANCHOR_RE
        .captures_iter(html)
        .map(|c| c[1].to_string())
        .filter(|href| is_product_href(href))
        .map(|href| absolutize(&href, base_url))
        .collect();
    dedup_in_order(anchors)
}

/// Heuristic for the anchor fallback: a `.html` page or a `/product` path
/// (Shopify uses `/products/{handle}`).
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

/// schema.org `offers` may be a single Offer or a list (per-variant); take the
/// first.
fn first_offer(offers: &Value) -> &Value {
    match offers {
        Value::Array(arr) => arr.first().unwrap_or(offers),
        _ => offers,
    }
}

fn gtin(node: &Value) -> Option<String> {
    ["gtin13", "gtin14", "gtin12", "gtin8", "gtin", "mpn"]
        .iter()
        .find_map(|k| node.get(*k).and_then(Value::as_str))
        .map(str::to_string)
}

/// schema.org availability → purchasable? Treat anything not explicitly
/// unavailable as in stock, so flash-sale states like `LimitedAvailability`
/// (Steep & Cheap) and `PreOrder` count as available.
fn availability_in_stock(availability: &str) -> bool {
    !["OutOfStock", "SoldOut", "Discontinued"]
        .iter()
        .any(|unavailable| availability.contains(unavailable))
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

fn dedup_in_order(urls: Vec<String>) -> Vec<String> {
    let mut seen = std::collections::HashSet::new();
    urls.into_iter()
        .filter(|u| seen.insert(u.clone()))
        .collect()
}
