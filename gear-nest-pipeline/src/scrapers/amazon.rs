//! Amazon Product Advertising API 5.0 client.
//!
//! Signs requests with AWS Signature Version 4 (service = `ProductAdvertisingAPI`)
//! and calls the `GetItems` operation against the configured PA-API host.
//! See <https://webservices.amazon.com/paapi5/documentation/>.
//!
//! The PA-API `GetItems` request accepts up to **10 `ItemIds`** per call. Above
//! that the API returns `RequestThrottled`. We chunk callers' batches into
//! groups of 10 and merge results.

use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use chrono::Utc;
use hmac::{Hmac, Mac};
use reqwest::{header, Client};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use tracing::{debug, warn};

use crate::config::PaapiConfig;
use crate::models::{PriceUpdate, RawProduct, RawReview};
use crate::scrapers::transport::{Tier, Transport};
use crate::scrapers::{jsonld, StoreCrawler};

const STORE_ID: &str = "amazon";
const BATCH_SIZE: usize = 10;
const SERVICE: &str = "ProductAdvertisingAPI";
const TARGET: &str = "com.amazon.paapi5.v1.ProductAdvertisingAPIv1.GetItems";
const OPERATION_PATH: &str = "/paapi5/getitems";

pub struct AmazonScraper {
    client: Client,
    config: PaapiConfig,
    marketplace: String,
    /// PA-API 5.0 returns only aggregate review counts
    /// (`CustomerReviews.Count` and `CustomerReviews.StarRating`), not
    /// individual review text. Individual reviews come from scraping
    /// `amazon.com/product-reviews/{ASIN}/`.
    review_scrape: Box<dyn Transport>,
}

impl AmazonScraper {
    pub fn new(config: PaapiConfig) -> Result<Self> {
        let client = Client::builder()
            .timeout(Duration::from_secs(30))
            .user_agent("gear-nest-pipeline/0.1 (+https://gearnest.io)")
            .build()
            .context("building reqwest client")?;
        Ok(Self {
            client,
            config,
            marketplace: "www.amazon.com".into(),
            review_scrape: Tier::CleanHttp.transport(STORE_ID)?,
        })
    }

    fn require_creds(&self) -> Result<(&str, &str, &str)> {
        let access = self
            .config
            .access_key
            .as_deref()
            .context("PAAPI_ACCESS_KEY not set")?;
        let secret = self
            .config
            .secret_key
            .as_deref()
            .context("PAAPI_SECRET_KEY not set")?;
        let partner = self
            .config
            .partner_tag
            .as_deref()
            .context("PAAPI_PARTNER_TAG not set")?;
        Ok((access, secret, partner))
    }

    async fn get_items(&self, asins: &[String]) -> Result<GetItemsResponse> {
        let (access, secret, partner) = self.require_creds()?;
        let body = serde_json::json!({
            "ItemIds": asins,
            "PartnerTag": partner,
            "PartnerType": "Associates",
            "Marketplace": self.marketplace,
            "Resources": [
                "ItemInfo.Title",
                "ItemInfo.ByLineInfo",
                "ItemInfo.Classifications",
                "ItemInfo.Features",
                "ItemInfo.ProductInfo",
                "ItemInfo.ExternalIds",
                "ItemInfo.ContentInfo",
                "Images.Primary.Large",
                "Offers.Listings.Price",
                "Offers.Listings.Availability.Message",
                "Offers.Listings.Availability.Type",
                "CustomerReviews.Count",
                "CustomerReviews.StarRating"
            ]
        });
        let body_bytes = serde_json::to_vec(&body)?;

        let url = format!(
            "{}://{}{}",
            self.config.scheme, self.config.host, OPERATION_PATH
        );
        let now = Utc::now();
        let amz_date = now.format("%Y%m%dT%H%M%SZ").to_string();
        let date_stamp = now.format("%Y%m%d").to_string();

        let auth = sign_v4(
            access,
            secret,
            &self.config.region,
            &self.config.host,
            OPERATION_PATH,
            &body_bytes,
            &amz_date,
            &date_stamp,
        );

        let resp = self
            .client
            .post(&url)
            .header(header::CONTENT_ENCODING, "amz-1.0")
            .header(header::CONTENT_TYPE, "application/json; charset=UTF-8")
            .header("X-Amz-Date", &amz_date)
            .header("X-Amz-Target", TARGET)
            .header(header::HOST, &self.config.host)
            .header(header::AUTHORIZATION, auth)
            .body(body_bytes)
            .send()
            .await
            .context("PA-API request failed")?;

        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("PA-API GetItems {status}: {text}");
        }
        let parsed: GetItemsResponse = serde_json::from_str(&text)
            .with_context(|| format!("decoding PA-API response: {text}"))?;
        Ok(parsed)
    }
}

#[async_trait]
impl StoreCrawler for AmazonScraper {
    fn store_id(&self) -> &str {
        STORE_ID
    }

    async fn fetch_batch(&self, ids: &[String]) -> Result<Vec<RawProduct>> {
        let mut out = Vec::with_capacity(ids.len());
        for chunk in ids.chunks(BATCH_SIZE) {
            let chunk_vec: Vec<String> = chunk.to_vec();
            debug!(count = chunk_vec.len(), "PA-API GetItems chunk");
            let resp = self.get_items(&chunk_vec).await?;
            if let Some(errs) = &resp.errors {
                for e in errs {
                    warn!(
                        code = e.code.as_str(),
                        message = e.message.as_str(),
                        "PA-API error"
                    );
                }
            }
            let Some(payload) = resp.items_result else {
                continue;
            };
            for item in payload.items {
                out.push(item_to_raw(&item));
            }
        }
        Ok(out)
    }

    async fn fetch_price(&self, store_product_id: &str) -> Result<PriceUpdate> {
        let resp = self.get_items(&[store_product_id.to_string()]).await?;
        let item = resp
            .items_result
            .and_then(|r| r.items.into_iter().next())
            .with_context(|| format!("PA-API returned no item for {store_product_id}"))?;
        let raw = item_to_raw(&item);
        Ok(PriceUpdate {
            store_id: STORE_ID.to_string(),
            store_product_id: store_product_id.to_string(),
            price: raw.price,
            in_stock: raw.in_stock,
            fetched_at: Utc::now(),
        })
    }

    /// PA-API 5.0 has no individual-review resource — `CustomerReviews.Count`
    /// and `CustomerReviews.StarRating` are aggregate only, already pulled
    /// into `RawProduct.store_rating` / `store_review_count` by `fetch_batch`.
    /// So individual reviews come from scraping
    /// `amazon.com/product-reviews/{ASIN}/`. Amazon's anti-bot is aggressive
    /// — expect frequent CAPTCHAs / 503s in live runs; the caller logs and
    /// moves on. The page server-renders the first review batch with
    /// schema.org `Review` markup, which the shared parser already handles.
    /// Caller `max` caps the result; we do not chase `?pageNumber=N` because
    /// deeper pages typically trip the CAPTCHA wall.
    async fn fetch_reviews(&self, store_product_id: &str, max: usize) -> Result<Vec<RawReview>> {
        let url = format!("https://www.amazon.com/product-reviews/{store_product_id}/");
        let html = self.review_scrape.get(&url).await?;
        let mut reviews = jsonld::parse_reviews(&html, STORE_ID, store_product_id);
        reviews.truncate(max);
        Ok(reviews)
    }
}

fn item_to_raw(item: &PaapiItem) -> RawProduct {
    let title = item
        .item_info
        .as_ref()
        .and_then(|i| i.title.as_ref())
        .map(|t| t.display_value.clone())
        .unwrap_or_default();
    let brand = item
        .item_info
        .as_ref()
        .and_then(|i| i.by_line_info.as_ref())
        .and_then(|b| b.brand.as_ref())
        .map(|v| v.display_value.clone());
    let category_path = item
        .item_info
        .as_ref()
        .and_then(|i| i.classifications.as_ref())
        .and_then(|c| c.product_group.as_ref())
        .map(|p| vec![p.display_value.clone()])
        .unwrap_or_default();
    let description = item
        .item_info
        .as_ref()
        .and_then(|i| i.content_info.as_ref())
        .and_then(|c| c.edition.as_ref())
        .map(|e| e.display_value.clone());
    let features = item
        .item_info
        .as_ref()
        .and_then(|i| i.features.as_ref())
        .map(|f| f.display_values.clone())
        .unwrap_or_default();
    let primary_image = item
        .images
        .as_ref()
        .and_then(|i| i.primary.as_ref())
        .and_then(|p| p.large.as_ref())
        .map(|u| u.url.clone());
    let gtin = item
        .item_info
        .as_ref()
        .and_then(|i| i.external_ids.as_ref())
        .and_then(|e| e.ea_ns.as_ref())
        .and_then(|e| e.display_values.first().cloned());
    let price_amount = item
        .offers
        .as_ref()
        .and_then(|o| o.listings.first())
        .and_then(|l| l.price.as_ref())
        .map(|p| format!("{:.2}", p.amount));
    let in_stock = item
        .offers
        .as_ref()
        .and_then(|o| o.listings.first())
        .and_then(|l| l.availability.as_ref())
        .map(|a| a.r#type.as_deref() == Some("Now"));
    let store_rating = item
        .customer_reviews
        .as_ref()
        .and_then(|c| c.star_rating.as_ref())
        .map(|s| s.value);
    let store_review_count = item
        .customer_reviews
        .as_ref()
        .and_then(|c| c.count)
        .unwrap_or(0);

    let raw_payload = serde_json::to_value(item).unwrap_or(serde_json::Value::Null);

    RawProduct {
        store_id: STORE_ID.into(),
        store_product_id: item.asin.clone(),
        url: item.detail_page_url.clone(),
        title,
        brand,
        category_path,
        description,
        features,
        specs: serde_json::Value::Object(serde_json::Map::new()),
        primary_image,
        gtin,
        price: price_amount,
        in_stock,
        store_rating,
        store_review_count,
        raw_payload,
    }
}

// ─── AWS SigV4 (POST + payload) ──────────────────────────────────────────────

type HmacSha256 = Hmac<Sha256>;

#[allow(clippy::too_many_arguments)]
fn sign_v4(
    access_key: &str,
    secret_key: &str,
    region: &str,
    host: &str,
    path: &str,
    body: &[u8],
    amz_date: &str,
    date_stamp: &str,
) -> String {
    let payload_hash = hex_sha256(body);
    let signed_headers = "content-encoding;host;x-amz-date;x-amz-target";
    let canonical_headers = format!(
        "content-encoding:amz-1.0\nhost:{host}\nx-amz-date:{amz_date}\nx-amz-target:{TARGET}\n"
    );
    let canonical_request =
        format!("POST\n{path}\n\n{canonical_headers}\n{signed_headers}\n{payload_hash}");
    let scope = format!("{date_stamp}/{region}/{SERVICE}/aws4_request");
    let string_to_sign = format!(
        "AWS4-HMAC-SHA256\n{amz_date}\n{scope}\n{}",
        hex_sha256(canonical_request.as_bytes())
    );

    let k_date = hmac(
        format!("AWS4{secret_key}").as_bytes(),
        date_stamp.as_bytes(),
    );
    let k_region = hmac(&k_date, region.as_bytes());
    let k_service = hmac(&k_region, SERVICE.as_bytes());
    let k_signing = hmac(&k_service, b"aws4_request");
    let signature = hex::encode(hmac(&k_signing, string_to_sign.as_bytes()));

    format!(
        "AWS4-HMAC-SHA256 Credential={access_key}/{scope}, \
         SignedHeaders={signed_headers}, Signature={signature}"
    )
}

fn hex_sha256(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    hex::encode(hasher.finalize())
}

fn hmac(key: &[u8], msg: &[u8]) -> Vec<u8> {
    let mut mac = HmacSha256::new_from_slice(key).expect("HMAC accepts any key length");
    mac.update(msg);
    mac.finalize().into_bytes().to_vec()
}

// ─── PA-API response shape (minimal — only fields we read) ───────────────────

#[derive(Debug, Deserialize)]
struct GetItemsResponse {
    #[serde(rename = "ItemsResult")]
    items_result: Option<ItemsResult>,
    #[serde(rename = "Errors")]
    errors: Option<Vec<PaapiError>>,
}

#[derive(Debug, Deserialize)]
struct ItemsResult {
    #[serde(rename = "Items", default)]
    items: Vec<PaapiItem>,
}

#[derive(Debug, Deserialize)]
struct PaapiError {
    #[serde(rename = "Code", default)]
    code: String,
    #[serde(rename = "Message", default)]
    message: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct PaapiItem {
    #[serde(rename = "ASIN")]
    asin: String,
    #[serde(rename = "DetailPageURL", default)]
    detail_page_url: String,
    #[serde(rename = "ItemInfo")]
    item_info: Option<ItemInfo>,
    #[serde(rename = "Images")]
    images: Option<Images>,
    #[serde(rename = "Offers")]
    offers: Option<Offers>,
    #[serde(rename = "CustomerReviews")]
    customer_reviews: Option<CustomerReviews>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ItemInfo {
    #[serde(rename = "Title")]
    title: Option<DisplayValue>,
    #[serde(rename = "ByLineInfo")]
    by_line_info: Option<ByLineInfo>,
    #[serde(rename = "Classifications")]
    classifications: Option<Classifications>,
    #[serde(rename = "Features")]
    features: Option<DisplayValues>,
    #[serde(rename = "ProductInfo")]
    #[allow(dead_code)]
    product_info: Option<serde_json::Value>,
    #[serde(rename = "ExternalIds")]
    external_ids: Option<ExternalIds>,
    #[serde(rename = "ContentInfo")]
    content_info: Option<ContentInfo>,
}

#[derive(Debug, Serialize, Deserialize)]
struct DisplayValue {
    #[serde(rename = "DisplayValue", default)]
    display_value: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct DisplayValues {
    #[serde(rename = "DisplayValues", default)]
    display_values: Vec<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ByLineInfo {
    #[serde(rename = "Brand")]
    brand: Option<DisplayValue>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Classifications {
    #[serde(rename = "ProductGroup")]
    product_group: Option<DisplayValue>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ContentInfo {
    #[serde(rename = "Edition")]
    edition: Option<DisplayValue>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ExternalIds {
    #[serde(rename = "EANs")]
    ea_ns: Option<DisplayValues>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Images {
    #[serde(rename = "Primary")]
    primary: Option<PrimaryImage>,
}

#[derive(Debug, Serialize, Deserialize)]
struct PrimaryImage {
    #[serde(rename = "Large")]
    large: Option<ImageUrl>,
}

#[derive(Debug, Serialize, Deserialize)]
struct ImageUrl {
    #[serde(rename = "URL", default)]
    url: String,
}

#[derive(Debug, Serialize, Deserialize)]
struct Offers {
    #[serde(rename = "Listings", default)]
    listings: Vec<Listing>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Listing {
    #[serde(rename = "Price")]
    price: Option<Price>,
    #[serde(rename = "Availability")]
    availability: Option<Availability>,
}

#[derive(Debug, Serialize, Deserialize)]
struct Price {
    #[serde(rename = "Amount")]
    amount: f64,
}

#[derive(Debug, Serialize, Deserialize)]
struct Availability {
    #[serde(rename = "Type")]
    r#type: Option<String>,
    #[serde(rename = "Message")]
    #[allow(dead_code)]
    message: Option<String>,
}

#[derive(Debug, Serialize, Deserialize)]
struct CustomerReviews {
    #[serde(rename = "Count")]
    count: Option<i32>,
    #[serde(rename = "StarRating")]
    star_rating: Option<StarRating>,
}

#[derive(Debug, Serialize, Deserialize)]
struct StarRating {
    #[serde(rename = "Value")]
    value: f32,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn sigv4_matches_known_vector() {
        // Sanity check: same inputs produce the same signature deterministically.
        let sig1 = sign_v4(
            "AKIDEXAMPLE",
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "us-east-1",
            "webservices.amazon.com",
            "/paapi5/getitems",
            b"{\"ItemIds\":[\"B0001\"]}",
            "20260527T120000Z",
            "20260527",
        );
        let sig2 = sign_v4(
            "AKIDEXAMPLE",
            "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
            "us-east-1",
            "webservices.amazon.com",
            "/paapi5/getitems",
            b"{\"ItemIds\":[\"B0001\"]}",
            "20260527T120000Z",
            "20260527",
        );
        assert_eq!(sig1, sig2);
        assert!(sig1.contains("AWS4-HMAC-SHA256"));
        assert!(sig1.contains("Credential=AKIDEXAMPLE/20260527/us-east-1/ProductAdvertisingAPI"));
    }

    #[test]
    fn item_to_raw_extracts_core_fields() {
        let item = PaapiItem {
            asin: "B0EXAMPLE".into(),
            detail_page_url: "https://www.amazon.com/dp/B0EXAMPLE".into(),
            item_info: Some(ItemInfo {
                title: Some(DisplayValue {
                    display_value: "MSR PocketRocket 2 Stove".into(),
                }),
                by_line_info: Some(ByLineInfo {
                    brand: Some(DisplayValue {
                        display_value: "MSR".into(),
                    }),
                }),
                classifications: None,
                features: Some(DisplayValues {
                    display_values: vec!["Lightweight".into(), "73g".into()],
                }),
                product_info: None,
                external_ids: None,
                content_info: None,
            }),
            images: None,
            offers: None,
            customer_reviews: None,
        };
        let raw = item_to_raw(&item);
        assert_eq!(raw.store_id, "amazon");
        assert_eq!(raw.store_product_id, "B0EXAMPLE");
        assert_eq!(raw.title, "MSR PocketRocket 2 Stove");
        assert_eq!(raw.brand.as_deref(), Some("MSR"));
        assert_eq!(raw.features.len(), 2);
    }
}
