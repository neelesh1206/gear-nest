use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use uuid::Uuid;

/// Raw scraped product. Free-form fields straight from the store's API/HTML.
/// Persisted as JSON in the scrape audit log; normalization happens downstream.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RawProduct {
    pub store_id: String,
    pub store_product_id: String,
    pub url: String,
    pub title: String,
    pub brand: Option<String>,
    pub category_path: Vec<String>,
    pub description: Option<String>,
    pub features: Vec<String>,
    pub specs: serde_json::Value,
    pub primary_image: Option<String>,
    pub gtin: Option<String>,
    /// Decimal as a fixed-point string (e.g. "129.99") so we can bind via `$N::numeric`
    /// without pulling in `rust_decimal` or `bigdecimal`.
    pub price: Option<String>,
    pub in_stock: Option<bool>,
    pub store_rating: Option<f32>,
    pub store_review_count: i32,
    pub raw_payload: serde_json::Value,
}

#[derive(Debug, Clone)]
pub struct NormalizedProduct {
    pub slug: String,
    pub name: String,
    pub brand: String,
    pub category: String,
    pub subcategory: Option<String>,
    pub description: Option<String>,
    pub specs: serde_json::Value,
    pub primary_image: Option<String>,
    pub gtin: Option<String>,
    pub canonical_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchConfidence {
    Exact,
    High,
    Medium,
    Candidate,
}

impl MatchConfidence {
    pub fn as_db_str(self) -> &'static str {
        match self {
            Self::Exact => "EXACT",
            Self::High => "HIGH",
            Self::Medium => "MEDIUM",
            Self::Candidate => "CANDIDATE",
        }
    }
}

#[derive(Debug, Clone)]
pub struct ResolvedProduct {
    pub product_id: Uuid,
    pub created: bool,
    pub confidence: MatchConfidence,
}

#[derive(Debug, Clone)]
pub struct PriceRecord {
    pub listing_id: Uuid,
    pub price: String,
    pub in_stock: Option<bool>,
    pub fetched_at: DateTime<Utc>,
}
