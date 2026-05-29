//! End-to-end: two fake crawlers (different stores) → run full-sync → assert
//! cross-store entity resolution collapsed them onto one canonical product.
//!
//! `#[ignore]` because it needs Postgres + Redis + a wiremock HF endpoint.
//! No live stores: both crawlers are injected via `run_with_crawlers`, so the
//! test is deterministic. It pins the load-bearing seam ADR-007 was always
//! about — two different stores naming the same product (`MSR PocketRocket 2`)
//! get one `products` row and two `store_listings`, with confidence high
//! enough that the API will surface them (EXACT | HIGH | MEDIUM).

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::too_many_lines
)]

use std::collections::HashMap;
use std::env;

use chrono::Utc;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use wiremock::matchers::method;
use wiremock::{Mock, MockServer, ResponseTemplate};

use gear_nest_pipeline::embeddings::HuggingFaceEmbedder;
use gear_nest_pipeline::models::{Category, PriceUpdate, RawProduct};
use gear_nest_pipeline::prices::PriceWriter;
use gear_nest_pipeline::scrapers::StoreCrawler;
use gear_nest_pipeline::{db, full_sync, price_history};

/// Fake crawler returning one fixture for one category. `store_id` and the
/// emitted `RawProduct.store_id` must be a real seeded store (FK to `stores.id`).
struct OneShotCrawler {
    store_id: &'static str,
    store_product_id: String,
    title: String,
    brand: String,
}

#[async_trait::async_trait]
impl StoreCrawler for OneShotCrawler {
    fn store_id(&self) -> &str {
        self.store_id
    }
    fn categories(&self) -> Vec<Category> {
        vec![Category {
            slug: "tents".into(),
            label: "Tents".into(),
        }]
    }
    async fn crawl_products(&self, _category: &Category) -> anyhow::Result<Vec<RawProduct>> {
        Ok(vec![RawProduct {
            store_id: self.store_id.to_string(),
            store_product_id: self.store_product_id.clone(),
            url: format!(
                "https://example.test/{}/{}",
                self.store_id, self.store_product_id
            ),
            title: self.title.clone(),
            brand: Some(self.brand.clone()),
            category_path: vec!["Camping".into(), "Stoves".into()],
            description: Some("Ultralight canister stove.".into()),
            features: vec!["Lightweight".into(), "73g".into()],
            specs: Value::Object(serde_json::Map::new()),
            primary_image: None,
            gtin: None,
            price: Some("49.95".into()),
            in_stock: Some(true),
            store_rating: None,
            store_review_count: 0,
            raw_payload: json!({"src": "fake"}),
        }])
    }
    async fn fetch_price(&self, _store_product_id: &str) -> anyhow::Result<PriceUpdate> {
        unreachable!("full-sync does not call fetch_price")
    }
}

#[tokio::test]
#[ignore = "requires Postgres + Redis; run with `cargo test -- --ignored`"]
async fn full_sync_resolves_cross_store_duplicates_to_one_product() {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let redis_url = env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into());
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .expect("postgres connect");

    let mig = db::locate_migrations_dir().expect("locate migrations");
    db::migrations::run(&pool, &mig).await.expect("migrate");
    price_history::ensure_partitions(&pool, Utc::now())
        .await
        .expect("partitions");

    // Unique fixture ids isolate this run from anything else in the DB.
    let run_tag = uuid::Uuid::new_v4().simple().to_string();
    let spid_a = format!("FSACT-A-{run_tag}");
    let spid_b = format!("FSACT-B-{run_tag}");
    let title = format!("MSR PocketRocket 2 Stove {run_tag}");

    clean_fixture_rows(&pool, &spid_a, &spid_b).await;

    // ─── HuggingFace stub: every request returns dim-384 zero vectors ─────
    let hf_mock = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(|req: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let n = body
                .get("inputs")
                .and_then(|v| v.as_array())
                .map_or(1, std::vec::Vec::len);
            let vectors: Vec<Vec<f32>> = (0..n).map(|_| vec![0.0_f32; 384]).collect();
            ResponseTemplate::new(200).set_body_json(vectors)
        })
        .mount(&hf_mock)
        .await;
    let embedder = HuggingFaceEmbedder::with_base_url(
        Some("hf-fake-token".into()),
        "BAAI/bge-small-en-v1.5".into(),
        Some(hf_mock.uri()),
    )
    .expect("embedder");

    let mut writer = PriceWriter::connect(&redis_url)
        .await
        .expect("redis connect");

    // Two real seeded store_ids (the FK on store_listings.store_id requires it).
    // Same brand + same model-digit token → same canonical_key → Tier-2 match.
    let mut crawlers: HashMap<&str, Box<dyn StoreCrawler>> = HashMap::new();
    crawlers.insert(
        "campsaver",
        Box::new(OneShotCrawler {
            store_id: "campsaver",
            store_product_id: spid_a.clone(),
            title: title.clone(),
            brand: "MSR".into(),
        }),
    );
    crawlers.insert(
        "garagerowngear",
        Box::new(OneShotCrawler {
            store_id: "garagerowngear",
            store_product_id: spid_b.clone(),
            title: title.clone(),
            brand: "Mountain Safety Research".into(),
        }),
    );

    let report = full_sync::run_with_crawlers(&pool, &mut writer, &embedder, &crawlers)
        .await
        .expect("full sync");
    assert_eq!(report.products, 2, "both crawled rows ingested");
    assert_eq!(report.failed, 0);

    // ─── Cross-store entity resolution (ADR-007) ─────────────────────────
    let (distinct_products,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT product_id) FROM store_listings \
         WHERE store_product_id IN ($1, $2)",
    )
    .bind(&spid_a)
    .bind(&spid_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        distinct_products, 1,
        "MSR + 'Mountain Safety Research' must collapse to one canonical product"
    );

    let (listing_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM store_listings WHERE store_product_id IN ($1, $2)")
            .bind(&spid_a)
            .bind(&spid_b)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(listing_count, 2, "one store_listing per store");

    let (active_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM store_listings \
         WHERE store_product_id IN ($1, $2) \
           AND match_confidence IN ('EXACT', 'HIGH', 'MEDIUM')",
    )
    .bind(&spid_a)
    .bind(&spid_b)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(
        active_count, 2,
        "both listings must be active (EXACT/HIGH/MEDIUM), not quarantined as CANDIDATE"
    );

    clean_fixture_rows(&pool, &spid_a, &spid_b).await;
}

async fn clean_fixture_rows(pool: &sqlx::PgPool, spid_a: &str, spid_b: &str) {
    sqlx::query(
        "DELETE FROM price_history WHERE listing_id IN \
         (SELECT id FROM store_listings WHERE store_product_id IN ($1, $2))",
    )
    .bind(spid_a)
    .bind(spid_b)
    .execute(pool)
    .await
    .ok();
    sqlx::query(
        "DELETE FROM products WHERE id IN \
         (SELECT product_id FROM store_listings WHERE store_product_id IN ($1, $2))",
    )
    .bind(spid_a)
    .bind(spid_b)
    .execute(pool)
    .await
    .ok();
    sqlx::query("DELETE FROM store_listings WHERE store_product_id IN ($1, $2)")
        .bind(spid_a)
        .bind(spid_b)
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM _gn_scrape_audit WHERE store_product_id IN ($1, $2)")
        .bind(spid_a)
        .bind(spid_b)
        .execute(pool)
        .await
        .ok();
}
