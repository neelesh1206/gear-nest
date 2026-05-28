//! Redis price cache writer with stale-while-revalidate semantics (ADR-009).
//!
//! Keys live indefinitely (no `EXPIRE`). Staleness is read by the API as the
//! age of an embedded `fetched_at` timestamp. First-write jitter (0–60 min
//! random offset) spreads the 24-hour staleness window across the catalog so
//! reads don't synchronize onto the morning batch run.
//!
//! Schema (Redis hash, key `prices:{product_id}`, field `{store_id}`):
//! ```json
//! { "price": "129.99", "in_stock": true, "fetched_at": "2026-05-27T12:34:56Z", "listing_id": "..." }
//! ```

use anyhow::{Context, Result};
use chrono::{DateTime, Duration as ChronoDuration, Utc};
use rand::Rng;
use redis::AsyncCommands;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tracing::{debug, instrument};
use uuid::Uuid;

/// Per ADR-009, staleness window = 24h. The jitter window of ±60min is added
/// only on the *first* write of a key, persisting the offset in the payload so
/// subsequent writes preserve the spread.
pub const STALE_AFTER: ChronoDuration = ChronoDuration::hours(24);
pub const JITTER_MAX: ChronoDuration = ChronoDuration::minutes(60);

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PricePayload {
    pub listing_id: Uuid,
    pub price: String,
    pub in_stock: Option<bool>,
    pub fetched_at: DateTime<Utc>,
    /// Random offset applied at first write (seconds, 0..=3600). Persisted so
    /// staleness checks across the lifetime of a key produce consistent decay.
    pub jitter_secs: i64,
}

impl PricePayload {
    pub fn is_stale(&self, now: DateTime<Utc>) -> bool {
        now - self.fetched_at > STALE_AFTER + ChronoDuration::seconds(self.jitter_secs)
    }
}

pub struct PriceWriter {
    conn: redis::aio::ConnectionManager,
}

impl PriceWriter {
    pub async fn connect(redis_url: &str) -> Result<Self> {
        let client = redis::Client::open(redis_url).context("parsing REDIS_URL")?;
        let conn = redis::aio::ConnectionManager::new(client)
            .await
            .context("opening Redis connection")?;
        Ok(Self { conn })
    }

    /// Upsert a price into the hash. If the field is new, generate jitter.
    /// If it exists, preserve the original `jitter_secs` so the staleness
    /// window stays anchored.
    #[instrument(skip(self), fields(product = %product_id, listing = %payload.listing_id))]
    pub async fn write(
        &mut self,
        product_id: Uuid,
        store_id: &str,
        mut payload: PricePayload,
    ) -> Result<()> {
        let key = price_key(product_id);
        let existing: Option<String> = self.conn.hget(&key, store_id).await?;
        if let Some(raw) = existing {
            if let Ok(prev) = serde_json::from_str::<PricePayload>(&raw) {
                payload.jitter_secs = prev.jitter_secs;
                debug!(jitter = payload.jitter_secs, "preserving first-write jitter");
            }
        } else {
            payload.jitter_secs = rand::thread_rng().gen_range(0..=JITTER_MAX.num_seconds());
            debug!(jitter = payload.jitter_secs, "new key — assigning jitter");
        }

        let value = serde_json::to_string(&json!({
            "listing_id": payload.listing_id,
            "price": payload.price,
            "in_stock": payload.in_stock,
            "fetched_at": payload.fetched_at,
            "jitter_secs": payload.jitter_secs,
        }))?;
        let _: () = self.conn.hset(&key, store_id, value).await?;
        Ok(())
    }

    pub async fn read(
        &mut self,
        product_id: Uuid,
        store_id: &str,
    ) -> Result<Option<PricePayload>> {
        let key = price_key(product_id);
        let raw: Option<String> = self.conn.hget(&key, store_id).await?;
        let Some(raw) = raw else { return Ok(None) };
        Ok(Some(serde_json::from_str(&raw)?))
    }
}

#[inline]
fn price_key(product_id: Uuid) -> String {
    format!("prices:{product_id}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn staleness_respects_jitter() {
        let now = Utc::now();
        let p = PricePayload {
            listing_id: Uuid::nil(),
            price: "1.00".into(),
            in_stock: Some(true),
            // exactly at the 24h boundary
            fetched_at: now - ChronoDuration::hours(24),
            jitter_secs: 1800, // +30min
        };
        // Without jitter we'd be stale; the 30min cushion keeps us fresh.
        assert!(!p.is_stale(now));
        let p_old = PricePayload {
            fetched_at: now - ChronoDuration::hours(25),
            ..p
        };
        assert!(p_old.is_stale(now));
    }
}
