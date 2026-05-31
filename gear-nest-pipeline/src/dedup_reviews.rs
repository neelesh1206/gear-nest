//! SPEC §13 Stage-1 cross-store review dedup.
//!
//! `UNIQUE(store_id, source_review_id)` already prevents same-store
//! re-imports. This pass handles the cross-store case: when the same
//! reviewer (`reviewer_id_hash`) reviewed the same canonical product at
//! two stores, keep the most credible row and drop the rest.
//!
//! Ranking, applied per `(product_id, reviewer_id_hash)` group:
//!   1. `verified_purchase = true` wins.
//!   2. Higher `helpful_votes` wins.
//!   3. More recent `review_date` wins.
//!   4. Stable `id` tiebreak so reruns are deterministic.
//!
//! Rows with `NULL reviewer_id_hash` (anonymous reviews) are untouched;
//! they're Stage-2's job (`MinHash` LSH, ADR-011, Phase 4).
//!
//! Runs as a single DELETE statement so concurrent writers see a
//! consistent collapse, and so re-running on a clean table is an O(1)
//! no-op rather than an O(n) re-scan.

use anyhow::{Context, Result};
use sqlx::postgres::PgPool;
use tracing::info;

#[derive(Debug, Default, Clone, Copy)]
pub struct DedupReviewsReport {
    pub deleted: u64,
}

pub async fn run(pool: &PgPool) -> Result<DedupReviewsReport> {
    let result = sqlx::query(
        "WITH ranked AS ( \
            SELECT id, \
                   ROW_NUMBER() OVER ( \
                       PARTITION BY product_id, reviewer_id_hash \
                       ORDER BY verified_purchase DESC, \
                                helpful_votes DESC, \
                                review_date DESC NULLS LAST, \
                                id \
                   ) AS rn \
            FROM reviews \
            WHERE reviewer_id_hash IS NOT NULL \
         ) \
         DELETE FROM reviews \
         WHERE id IN (SELECT id FROM ranked WHERE rn > 1)",
    )
    .execute(pool)
    .await
    .context("dedup-reviews delete")?;

    let report = DedupReviewsReport {
        deleted: result.rows_affected(),
    };
    info!(
        deleted = report.deleted,
        "dedup-reviews complete (SPEC §13 Stage 1)"
    );
    Ok(report)
}
