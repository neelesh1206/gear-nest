//! Idempotent persistence of scraped reviews into the `reviews` table.
//!
//! `UNIQUE(store_id, source_review_id)` (migration 0001) handles same-store
//! re-imports; we `ON CONFLICT` refresh every mutable field (`rating`,
//! `title`, `body`, `verified_purchase`, `helpful_votes`, `review_date`,
//! `scraped_at`) so an edit at the source (Amazon lets reviewers update
//! their text; helpful counts drift) lands on the next re-scrape. Identity
//! columns (`product_id`, `reviewer_id_hash`) stay put. Cross-store
//! reviewer dedup is a separate pass (Phase 3 PR 7 / SPEC §13 Stage 1).

use anyhow::Result;
use sqlx::postgres::PgPool;
use uuid::Uuid;

use crate::models::RawReview;

/// Batch-upsert reviews for a known product. Returns the number of rows
/// affected (inserts + updates).
pub async fn upsert_batch(pool: &PgPool, product_id: Uuid, raws: &[RawReview]) -> Result<u64> {
    if raws.is_empty() {
        return Ok(0);
    }
    let mut sql = String::from(
        "INSERT INTO reviews (\
            product_id, store_id, source_review_id, reviewer_id_hash, \
            rating, title, body, verified_purchase, helpful_votes, review_date\
         ) VALUES ",
    );
    let mut placeholders: Vec<String> = Vec::with_capacity(raws.len());
    for i in 0..raws.len() {
        let b = i * 10;
        placeholders.push(format!(
            "(${},${},${},${},${},${},${},${},${},${})",
            b + 1,
            b + 2,
            b + 3,
            b + 4,
            b + 5,
            b + 6,
            b + 7,
            b + 8,
            b + 9,
            b + 10,
        ));
    }
    sql.push_str(&placeholders.join(","));
    sql.push_str(
        " ON CONFLICT (store_id, source_review_id) DO UPDATE \
         SET rating = EXCLUDED.rating, \
             title = EXCLUDED.title, \
             body = EXCLUDED.body, \
             verified_purchase = EXCLUDED.verified_purchase, \
             helpful_votes = EXCLUDED.helpful_votes, \
             review_date = EXCLUDED.review_date, \
             scraped_at = NOW()",
    );

    let mut q = sqlx::query(&sql);
    for r in raws {
        q = q
            .bind(product_id)
            .bind(&r.store_id)
            .bind(&r.source_review_id)
            .bind(&r.reviewer_id_hash)
            .bind(r.rating)
            .bind(&r.title)
            .bind(&r.body)
            .bind(r.verified_purchase)
            .bind(r.helpful_votes)
            .bind(r.review_date);
    }
    let result = q.execute(pool).await?;
    Ok(result.rows_affected())
}
