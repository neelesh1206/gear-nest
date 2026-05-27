//! End-to-end: scrape 50 Amazon products → normalize → resolve → embed → DB.
//!
//! Marked `#[ignore]` because it needs a running Postgres + Redis. CI invokes
//! it explicitly with `cargo test -- --ignored`. The PA-API and HuggingFace
//! endpoints are stubbed with `wiremock`, so the test does not need real
//! Amazon/HF credentials.
//!
//! Required env (compose-up defaults work):
//!     DATABASE_URL=postgresql://gearnest:gearnest_dev@localhost:5432/gearnest
//!     REDIS_URL=redis://localhost:6379

use std::env;

use chrono::Utc;
use serde_json::{json, Value};
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use gear_nest_pipeline::{
    config::PaapiConfig,
    db,
    embeddings::HuggingFaceEmbedder,
    entity_resolution::Resolver,
    normalizer,
    price_history,
    scrapers::{amazon::AmazonScraper, ensure_scrape_audit, record_raw, StoreCrawler},
};

const PRODUCT_COUNT: usize = 50;

#[tokio::test]
#[ignore = "requires Postgres + Redis; run with `cargo test -- --ignored`"]
async fn scrape_50_amazon_products_end_to_end() {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .expect("postgres connect");

    // Apply migrations + partition DDL so the test is self-contained on a fresh DB.
    let mig_dir = db::locate_migrations_dir().expect("locate migrations dir");
    db::migrations::run(&pool, &mig_dir).await.expect("migrate");
    price_history::ensure_partitions(&pool, Utc::now())
        .await
        .expect("partitions");
    ensure_scrape_audit(&pool).await.expect("audit table");

    // Isolate this test run: wipe pipeline-touched rows for our fixture ASINs.
    let asins: Vec<String> = (1..=PRODUCT_COUNT)
        .map(|n| format!("B0FIXT{n:04}"))
        .collect();
    clean_fixture_rows(&pool, &asins).await;

    // ─── PA-API stub ─────────────────────────────────────────────────────
    let paapi_mock = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/paapi5/getitems"))
        .respond_with(|req: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let requested: Vec<String> = body
                .get("ItemIds")
                .and_then(|v| v.as_array())
                .map(|arr| arr.iter().filter_map(|x| x.as_str().map(str::to_string)).collect())
                .unwrap_or_default();
            ResponseTemplate::new(200).set_body_json(fake_paapi_response(&requested))
        })
        .mount(&paapi_mock)
        .await;

    // wiremock URI is `http://127.0.0.1:NNNNN`; strip the scheme to get host:port.
    let paapi_uri = paapi_mock.uri();
    let host_port = paapi_uri
        .strip_prefix("http://")
        .or_else(|| paapi_uri.strip_prefix("https://"))
        .expect("wiremock URI has an http(s) scheme")
        .to_string();
    let paapi_cfg = PaapiConfig {
        access_key: Some("AKIDFAKE".into()),
        secret_key: Some("SECRETFAKE".into()),
        partner_tag: Some("gearnest-20".into()),
        host: host_port,
        region: "us-east-1".into(),
        scheme: "http".into(),
    };
    let scraper = AmazonScraper::new(paapi_cfg).expect("scraper");

    // ─── HuggingFace stub: every request returns dim-384 zero vectors ─────
    let hf_mock = MockServer::start().await;
    Mock::given(method("POST"))
        .respond_with(|req: &wiremock::Request| {
            let body: Value = serde_json::from_slice(&req.body).unwrap_or(Value::Null);
            let n = body
                .get("inputs")
                .and_then(|v| v.as_array())
                .map(|a| a.len())
                .unwrap_or(1);
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

    // ─── Pipeline run ────────────────────────────────────────────────────
    let resolver = Resolver::new(&pool);
    let raws = scraper.fetch_batch(&asins).await.expect("scrape");
    assert_eq!(raws.len(), PRODUCT_COUNT, "PA-API returned all 50 fixtures");

    for raw in &raws {
        record_raw(&pool, raw).await.expect("audit");
        let norm = normalizer::normalize(raw);
        let resolved = resolver.resolve(raw, &norm).await.expect("resolve");
        gear_nest_pipeline::embeddings::embed_and_insert_product_specs(
            &embedder,
            &pool,
            resolved.product_id,
            norm.description.as_deref(),
            norm.specs
                .get("features")
                .and_then(|v| v.as_array())
                .map(|arr| {
                    arr.iter()
                        .filter_map(|x| x.as_str().map(str::to_string))
                        .collect::<Vec<_>>()
                })
                .unwrap_or_default()
                .as_slice(),
        )
        .await
        .expect("embed specs");
    }

    // ─── Assertions ──────────────────────────────────────────────────────
    let (audit_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM _gn_scrape_audit WHERE store_id = 'amazon' AND store_product_id = ANY($1)")
            .bind(&asins)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(audit_count, PRODUCT_COUNT as i64);

    let (listing_count,): (i64,) =
        sqlx::query_as("SELECT COUNT(*) FROM store_listings WHERE store_id = 'amazon' AND store_product_id = ANY($1)")
            .bind(&asins)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(listing_count, PRODUCT_COUNT as i64);

    let (product_count,): (i64,) = sqlx::query_as(
        "SELECT COUNT(DISTINCT product_id) FROM store_listings \
         WHERE store_id = 'amazon' AND store_product_id = ANY($1)",
    )
    .bind(&asins)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(product_count, PRODUCT_COUNT as i64, "one product per ASIN");

    let (spec_chunks,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM spec_chunks WHERE product_id IN \
         (SELECT product_id FROM store_listings WHERE store_id = 'amazon' AND store_product_id = ANY($1))",
    )
    .bind(&asins)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert!(spec_chunks >= PRODUCT_COUNT as i64, "≥1 spec chunk per product");

    // Confidence distribution: all 50 are net-new, so all EXACT.
    let (exact,): (i64,) = sqlx::query_as(
        "SELECT COUNT(*) FROM store_listings WHERE store_id = 'amazon' \
         AND store_product_id = ANY($1) AND match_confidence = 'EXACT'",
    )
    .bind(&asins)
    .fetch_one(&pool)
    .await
    .unwrap();
    assert_eq!(exact, PRODUCT_COUNT as i64);
}

async fn clean_fixture_rows(pool: &sqlx::PgPool, asins: &[String]) {
    sqlx::query(
        "DELETE FROM products WHERE id IN \
         (SELECT product_id FROM store_listings WHERE store_id = 'amazon' AND store_product_id = ANY($1))",
    )
    .bind(asins)
    .execute(pool)
    .await
    .ok();
    sqlx::query("DELETE FROM store_listings WHERE store_id = 'amazon' AND store_product_id = ANY($1)")
        .bind(asins)
        .execute(pool)
        .await
        .ok();
    sqlx::query("DELETE FROM _gn_scrape_audit WHERE store_id = 'amazon' AND store_product_id = ANY($1)")
        .bind(asins)
        .execute(pool)
        .await
        .ok();
}

fn fake_paapi_response(requested: &[String]) -> Value {
    let brands = [
        "MSR",
        "Patagonia",
        "Arc'teryx",
        "Mountain Safety Research", // alias of MSR — exercises Tier-2 normalization
        "Big Agnes",
        "Black Diamond",
        "Hoka",
        "Salomon",
        "Osprey",
        "Nemo",
    ];
    let categories = [
        "Sports & Outdoors > Camping & Hiking > Tents",
        "Sports & Outdoors > Camping & Hiking > Stoves",
        "Sports & Outdoors > Backpacks > Hiking Daypacks",
        "Sports & Outdoors > Apparel > Jackets",
        "Sports & Outdoors > Sleeping Bags",
        "Sports & Outdoors > Footwear > Trail Running",
    ];

    let items: Vec<Value> = requested
        .iter()
        .enumerate()
        .map(|(idx, asin)| {
            let brand = brands[idx % brands.len()];
            let cat = categories[idx % categories.len()];
            let price = 19.99 + (idx as f64) * 7.5;
            json!({
                "ASIN": asin,
                "DetailPageURL": format!("https://www.amazon.com/dp/{asin}"),
                "ItemInfo": {
                    "Title": { "DisplayValue": format!("{brand} Test Item {idx} Model X{idx:03}") },
                    "ByLineInfo": { "Brand": { "DisplayValue": brand } },
                    "Classifications": { "ProductGroup": { "DisplayValue": cat } },
                    "Features": {
                        "DisplayValues": [
                            format!("Weight: {} oz", 10 + idx),
                            "Waterproof construction",
                            "Lifetime warranty"
                        ]
                    },
                    "ExternalIds": {
                        "EANs": { "DisplayValues": [format!("8901234{:06}", idx)] }
                    }
                },
                "Images": { "Primary": { "Large": { "URL": format!("https://m.media-amazon.com/{asin}.jpg") } } },
                "Offers": {
                    "Listings": [{
                        "Price": { "Amount": price, "Currency": "USD", "DisplayAmount": format!("${price:.2}") },
                        "Availability": { "Type": "Now", "Message": "In Stock." }
                    }]
                },
                "CustomerReviews": { "Count": 100 + idx as i32, "StarRating": { "Value": 4.0 + (idx as f32 % 10.0) * 0.1 } }
            })
        })
        .collect();

    json!({ "ItemsResult": { "Items": items } })
}
