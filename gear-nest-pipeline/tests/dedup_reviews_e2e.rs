//! End-to-end: seed reviews with deliberate cross-store dupes → run
//! dedup-reviews → assert the right rows survive.
//!
//! `#[ignore]` because it needs Postgres (CI runs it with `--ignored`
//! against the service container). Pins the SPEC §13 Stage-1 ranking:
//! same `(product_id, reviewer_id_hash)` across two stores collapses to
//! one row, with `verified_purchase` winning over `helpful_votes` over
//! `review_date`. Anonymous rows (`reviewer_id_hash IS NULL`) are
//! untouched — that's Stage-2's job (ADR-011, Phase 4).

#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::too_many_lines
)]

use std::env;

use chrono::NaiveDate;
use sqlx::postgres::PgPoolOptions;
use uuid::Uuid;

use gear_nest_pipeline::{db, dedup_reviews};

#[tokio::test]
#[ignore = "requires Postgres; run with `cargo test -- --ignored`"]
async fn dedup_collapses_cross_store_same_reviewer_and_spares_anon() {
    let database_url = env::var("DATABASE_URL").expect("DATABASE_URL must be set");
    let pool = PgPoolOptions::new()
        .max_connections(4)
        .connect(&database_url)
        .await
        .expect("postgres connect");
    let mig = db::locate_migrations_dir().expect("locate migrations");
    db::migrations::run(&pool, &mig).await.expect("migrate");

    let product_id = Uuid::new_v4();
    let slug = format!("dr-test-{product_id}");

    sqlx::query(
        "INSERT INTO products (id, slug, name, brand, category) \
         VALUES ($1, $2, 'DR Test Tent', 'TestBrand', 'shelter')",
    )
    .bind(product_id)
    .bind(&slug)
    .execute(&pool)
    .await
    .expect("seed product");

    // Sarah K. reviewed the same product on Amazon + REI. Amazon row is
    // verified_purchase = true → wins.
    let sarah_hash = "hash-sarah-k";
    let loser_id = insert_review(
        &pool,
        product_id,
        "rei",
        "rei-r-001",
        Some(sarah_hash),
        5,
        false,
        20,
        NaiveDate::from_ymd_opt(2025, 9, 1),
    )
    .await;
    let winner_id = insert_review(
        &pool,
        product_id,
        "amazon",
        "amz-r-001",
        Some(sarah_hash),
        5,
        true, // verified_purchase
        2,
        NaiveDate::from_ymd_opt(2025, 7, 1),
    )
    .await;

    // Mike R. on two stores, neither verified. helpful_votes tiebreak: REI
    // wins (15 > 3).
    let mike_hash = "hash-mike-r";
    let mike_loser_id = insert_review(
        &pool,
        product_id,
        "amazon",
        "amz-r-002",
        Some(mike_hash),
        4,
        false,
        3,
        NaiveDate::from_ymd_opt(2025, 8, 10),
    )
    .await;
    let mike_winner_id = insert_review(
        &pool,
        product_id,
        "rei",
        "rei-r-002",
        Some(mike_hash),
        4,
        false,
        15,
        NaiveDate::from_ymd_opt(2025, 8, 1),
    )
    .await;

    // Two anonymous (NULL hash) rows on the same product — Stage 1 must
    // leave both in place. Stage 2 (MinHash, Phase 4) handles these.
    let anon_a = insert_review(
        &pool,
        product_id,
        "campsaver",
        "cs-anon-1",
        None,
        4,
        false,
        0,
        NaiveDate::from_ymd_opt(2025, 6, 1),
    )
    .await;
    let anon_b = insert_review(
        &pool,
        product_id,
        "garagerowngear",
        "ggg-anon-1",
        None,
        4,
        false,
        0,
        NaiveDate::from_ymd_opt(2025, 6, 5),
    )
    .await;

    // Unique reviewer (no collision) on one store — survives unchanged.
    let solo_id = insert_review(
        &pool,
        product_id,
        "moosejaw",
        "mj-r-001",
        Some("hash-unique-reviewer"),
        5,
        false,
        7,
        NaiveDate::from_ymd_opt(2025, 7, 15),
    )
    .await;

    let report = dedup_reviews::run(&pool).await.expect("dedup");
    assert_eq!(report.deleted, 2, "two cross-store dupes should collapse");

    // Survivors: winners + anon pair + solo.
    let surviving = surviving_ids(&pool, product_id).await;
    assert!(
        surviving.contains(&winner_id),
        "sarah amazon (verified) kept"
    );
    assert!(!surviving.contains(&loser_id), "sarah rei dropped");
    assert!(
        surviving.contains(&mike_winner_id),
        "mike rei (more helpful_votes) kept"
    );
    assert!(!surviving.contains(&mike_loser_id), "mike amazon dropped");
    assert!(surviving.contains(&anon_a), "anon A spared (NULL hash)");
    assert!(surviving.contains(&anon_b), "anon B spared (NULL hash)");
    assert!(surviving.contains(&solo_id), "unique reviewer survives");

    // Idempotent: re-running on the cleaned table deletes nothing.
    let report2 = dedup_reviews::run(&pool).await.expect("dedup rerun");
    assert_eq!(report2.deleted, 0, "rerun is a no-op");
}

#[allow(clippy::too_many_arguments)]
async fn insert_review(
    pool: &sqlx::PgPool,
    product_id: Uuid,
    store_id: &str,
    source_review_id: &str,
    reviewer_id_hash: Option<&str>,
    rating: i16,
    verified_purchase: bool,
    helpful_votes: i32,
    review_date: Option<NaiveDate>,
) -> Uuid {
    sqlx::query_scalar(
        "INSERT INTO reviews ( \
            product_id, store_id, source_review_id, reviewer_id_hash, \
            rating, title, body, verified_purchase, helpful_votes, review_date \
         ) VALUES ($1,$2,$3,$4,$5,'t','body of the review for dedup test',$6,$7,$8) \
         RETURNING id",
    )
    .bind(product_id)
    .bind(store_id)
    .bind(source_review_id)
    .bind(reviewer_id_hash)
    .bind(rating)
    .bind(verified_purchase)
    .bind(helpful_votes)
    .bind(review_date)
    .fetch_one(pool)
    .await
    .expect("insert review")
}

async fn surviving_ids(pool: &sqlx::PgPool, product_id: Uuid) -> std::collections::HashSet<Uuid> {
    let rows: Vec<(Uuid,)> = sqlx::query_as("SELECT id FROM reviews WHERE product_id = $1")
        .bind(product_id)
        .fetch_all(pool)
        .await
        .unwrap();
    rows.into_iter().map(|(id,)| id).collect()
}
