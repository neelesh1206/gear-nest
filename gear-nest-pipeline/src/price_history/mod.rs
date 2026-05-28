//! `price_history` append with month-partition awareness (ADR-010).
//!
//! Session 0's migration seeds partitions for 2026-05/06/07. Beyond that
//! horizon the pipeline must create partitions itself at startup, otherwise
//! the first insert past month rollover fails with `no partition of relation
//! "price_history" found for row`.
//!
//! [`ensure_partitions`] is idempotent: it creates the current month and the
//! next two months ahead, using `CREATE TABLE IF NOT EXISTS`. Safe to call on
//! every boot.

use anyhow::{Context, Result};
use chrono::{DateTime, Datelike, NaiveDate, Utc};
use sqlx::postgres::PgPool;
use tracing::{debug, info};
use uuid::Uuid;

use crate::models::PriceRecord;

/// Ensure month partitions exist for [now-1, now, now+1, now+2]. The
/// trailing two months give the daily price-sync job ~60 days of runway
/// without needing a separate DDL job.
pub async fn ensure_partitions(pool: &PgPool, anchor: DateTime<Utc>) -> Result<()> {
    let anchor_date = anchor.date_naive();
    let months = [
        previous_month_start(anchor_date),
        month_start(anchor_date),
        add_months(month_start(anchor_date), 1),
        add_months(month_start(anchor_date), 2),
    ];

    for start in months {
        let end = add_months(start, 1);
        let name = format!("price_history_{}_{:02}", start.year(), start.month());
        let sql = format!(
            "CREATE TABLE IF NOT EXISTS {name} \
             PARTITION OF price_history \
             FOR VALUES FROM ('{start}') TO ('{end}')"
        );
        debug!(partition = %name, "ensure_partitions DDL");
        sqlx::query(&sql)
            .execute(pool)
            .await
            .with_context(|| format!("creating partition {name}"))?;
    }
    info!("price_history partitions ensured (current ±1, +2)");
    Ok(())
}

/// Append a single price observation. The partition router uses `fetched_at`.
pub async fn append(pool: &PgPool, rec: &PriceRecord) -> Result<i64> {
    let row: (i64,) = sqlx::query_as(
        "INSERT INTO price_history (listing_id, price, in_stock, fetched_at) \
         VALUES ($1, $2::numeric, $3, $4) RETURNING id",
    )
    .bind(rec.listing_id)
    .bind(&rec.price)
    .bind(rec.in_stock)
    .bind(rec.fetched_at)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

/// Batched append. One round trip per N records. Use this from the daily job
/// rather than calling [`append`] in a loop.
pub async fn append_many(pool: &PgPool, recs: &[PriceRecord]) -> Result<u64> {
    if recs.is_empty() {
        return Ok(0);
    }
    let mut sql =
        String::from("INSERT INTO price_history (listing_id, price, in_stock, fetched_at) VALUES ");
    let mut placeholders: Vec<String> = Vec::with_capacity(recs.len());
    for i in 0..recs.len() {
        let b = i * 4;
        placeholders.push(format!(
            "(${},${}::numeric,${},${})",
            b + 1,
            b + 2,
            b + 3,
            b + 4
        ));
    }
    sql.push_str(&placeholders.join(","));

    let mut q = sqlx::query(&sql);
    for r in recs {
        q = q
            .bind(r.listing_id)
            .bind(&r.price)
            .bind(r.in_stock)
            .bind(r.fetched_at);
    }
    let result = q.execute(pool).await?;
    Ok(result.rows_affected())
}

/// Lookup the most recent price observation for a listing. Used to drive the
/// Redis fallback path (ADR-006).
pub async fn latest_for_listing(pool: &PgPool, listing_id: Uuid) -> Result<Option<PriceRecord>> {
    let row: Option<(Uuid, String, Option<bool>, DateTime<Utc>)> = sqlx::query_as(
        "SELECT listing_id, price::text, in_stock, fetched_at \
         FROM price_history \
         WHERE listing_id = $1 \
         ORDER BY fetched_at DESC \
         LIMIT 1",
    )
    .bind(listing_id)
    .fetch_optional(pool)
    .await?;
    Ok(
        row.map(|(listing_id, price, in_stock, fetched_at)| PriceRecord {
            listing_id,
            price,
            in_stock,
            fetched_at,
        }),
    )
}

fn month_start(d: NaiveDate) -> NaiveDate {
    NaiveDate::from_ymd_opt(d.year(), d.month(), 1).expect("year/month always yields a valid 1st")
}

fn previous_month_start(d: NaiveDate) -> NaiveDate {
    add_months(month_start(d), -1)
}

// `month()` is always 1..=12 and the loops keep `month` in 1..=12, so the
// u32<->i32 casts can neither wrap nor lose sign.
#[allow(clippy::cast_possible_wrap, clippy::cast_sign_loss)]
fn add_months(d: NaiveDate, months: i32) -> NaiveDate {
    let mut year = d.year();
    let mut month = d.month() as i32 + months;
    while month <= 0 {
        month += 12;
        year -= 1;
    }
    while month > 12 {
        month -= 12;
        year += 1;
    }
    NaiveDate::from_ymd_opt(year, month as u32, 1)
        .expect("add_months always yields a valid month 1st")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn month_math_handles_year_boundary() {
        let dec = NaiveDate::from_ymd_opt(2026, 12, 15).unwrap();
        let jan = add_months(month_start(dec), 1);
        assert_eq!(jan, NaiveDate::from_ymd_opt(2027, 1, 1).unwrap());

        let jan_first = NaiveDate::from_ymd_opt(2027, 1, 10).unwrap();
        let prev = previous_month_start(jan_first);
        assert_eq!(prev, NaiveDate::from_ymd_opt(2026, 12, 1).unwrap());
    }
}
