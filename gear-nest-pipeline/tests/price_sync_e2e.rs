//! End-to-end: seed listings → run price-sync → assert Redis + `price_history`.
//!
//! `#[ignore]` because it needs Postgres + Redis (CI runs it with `--ignored`
//! against service containers). No live stores: a fake `StoreCrawler` is
//! injected via `run_with_crawlers`, so the test is deterministic. It pins the
//! two load-bearing behaviors: CANDIDATE listings are excluded (ADR-007), and a
//! synced price lands in both Redis and `price_history`.

use std::collections::HashMap;
use std::env;

use chrono::Utc;
use redis::AsyncCommands;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use gear_nest_pipeline::models::PriceUpdate;
use gear_nest_pipeline::prices::PriceWriter;
use gear_nest_pipeline::scrapers::StoreCrawler;
use gear_nest_pipeline::{db, price_history, price_sync};

const FAKE_STORE: &str = "campsaver";

struct FakeStore;

#[async_trait::async_trait]
impl StoreCrawler for FakeStore {
    fn store_id(&self) -> &str {
        FAKE_STORE
    }
    async fn fetch_price(&self, store_product_id: &str) -> anyhow::Result<PriceUpdate> {
        Ok(PriceUpdate {
            store_id: FAKE_STORE.into(),
            store_product_id: store_product_id.into(),
            price: Some("142.50".into()),
            in_stock: Some(true),
            fetched_at: Utc::now(),
        })
    }
}

#[tokio::test]
#[ignore = "requires Postgres + Redis; run with `cargo test -- --ignored`"]
async fn price_sync_writes_active_listings_and_skips_candidate() {
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
    let product_id = Uuid::new_v4();
    let slug = format!("ps-test-{product_id}");
    let active_spid = format!("PSACT-{product_id}");
    let candidate_spid = format!("PSCAND-{product_id}");

    sqlx::query(
        "INSERT INTO products (id, slug, name, brand, category) \
         VALUES ($1, $2, 'PS Test Tent', 'TestBrand', 'shelter')",
    )
    .bind(product_id)
    .bind(&slug)
    .execute(&pool)
    .await
    .expect("seed product");

    let active_listing_id: Uuid = sqlx::query_scalar(
        "INSERT INTO store_listings (product_id, store_id, store_product_id, store_url, match_confidence) \
         VALUES ($1, 'campsaver', $2, 'https://www.campsaver.com/active', 'EXACT') RETURNING id",
    )
    .bind(product_id)
    .bind(&active_spid)
    .fetch_one(&pool)
    .await
    .expect("seed active listing");

    let candidate_listing_id: Uuid = sqlx::query_scalar(
        "INSERT INTO store_listings (product_id, store_id, store_product_id, store_url, match_confidence) \
         VALUES ($1, 'campsaver', $2, 'https://www.campsaver.com/candidate', 'CANDIDATE') RETURNING id",
    )
    .bind(product_id)
    .bind(&candidate_spid)
    .fetch_one(&pool)
    .await
    .expect("seed candidate listing");

    let mut writer = PriceWriter::connect(&redis_url)
        .await
        .expect("redis connect");
    let mut crawlers: HashMap<&str, Box<dyn StoreCrawler>> = HashMap::new();
    crawlers.insert("campsaver", Box::new(FakeStore));

    let report = price_sync::run_with_crawlers(&pool, &mut writer, &crawlers)
        .await
        .expect("price sync");
    assert!(report.synced >= 1, "our EXACT listing should have synced");

    // price_history: one row for the active listing, none for the candidate.
    let active_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM price_history WHERE listing_id = $1")
            .bind(active_listing_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(active_rows, 1, "active listing written to price_history");

    let candidate_rows: i64 =
        sqlx::query_scalar("SELECT COUNT(*) FROM price_history WHERE listing_id = $1")
            .bind(candidate_listing_id)
            .fetch_one(&pool)
            .await
            .unwrap();
    assert_eq!(candidate_rows, 0, "CANDIDATE listing excluded (ADR-007)");

    // Redis: the price is readable for this product/store.
    let payload = writer
        .read(product_id, "campsaver")
        .await
        .unwrap()
        .expect("redis price present");
    assert_eq!(payload.price, "142.50");
    assert_eq!(payload.listing_id, active_listing_id);

    // last-updated marker stamped.
    let last_updated: Option<String> = redis::Client::open(redis_url)
        .unwrap()
        .get_multiplexed_async_connection()
        .await
        .unwrap()
        .get("prices:last_updated")
        .await
        .unwrap();
    assert!(last_updated.is_some(), "prices:last_updated stamped");

    // cleanup
    sqlx::query("DELETE FROM price_history WHERE listing_id IN ($1, $2)")
        .bind(active_listing_id)
        .bind(candidate_listing_id)
        .execute(&pool)
        .await
        .ok();
    sqlx::query("DELETE FROM store_listings WHERE product_id = $1")
        .bind(product_id)
        .execute(&pool)
        .await
        .ok();
    sqlx::query("DELETE FROM products WHERE id = $1")
        .bind(product_id)
        .execute(&pool)
        .await
        .ok();
}
