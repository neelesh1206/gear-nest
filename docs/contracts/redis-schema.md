# Redis Schema Contract

> Cross-service contract. The Rust pipeline is the **writer**; the Java API is the
> **reader**. Changes here require updating both services in the same PR (or
> coordinated PRs) and must be reviewed by the contract owner (Session 0).

This artifact exists because the price schema previously lived only in prose
(SPEC §6-7). The pipeline and API interpreted it differently, silently breaking
the price cache (the API read a key the pipeline never wrote). This file is now
the single source of truth.

---

## Price cache (stale-while-revalidate — ADR-006, ADR-009)

| | |
|---|---|
| **Key** | `prices:{product_id}` |
| **Type** | Hash |
| **Field** | `{store_id}` (e.g. `amazon`, `rei`, `backcountry`) |
| **Value** | JSON string (see below) |
| **TTL** | **None.** Keys are permanent; staleness is application-managed (ADR-009). |

### Value (JSON)

```json
{
  "listing_id": "uuid",
  "price": "129.99",
  "in_stock": true,
  "fetched_at": "2026-05-27T12:34:56Z",
  "jitter_secs": 1380
}
```

| Field | Type | Notes |
|-------|------|-------|
| `listing_id` | UUID string | The `store_listings.id` this price belongs to. |
| `price` | string | Decimal as a string to avoid float drift. Reader parses to float. |
| `in_stock` | bool \| null | |
| `fetched_at` | RFC3339 / ISO-8601 UTC | When the pipeline fetched the price. Reader parses as `OffsetDateTime`. |
| `jitter_secs` | int (0–3600) | First-write random offset; spreads the 24h staleness window across the catalog. Set once, preserved on subsequent writes. |

### Writer (pipeline)
`gear-nest-pipeline/src/prices/mod.rs` — `PriceWriter::write(product_id, store_id, payload)`.

### Reader (API)
`gear-nest-api/.../pricing/PricingService.java` — `readSnapshot(productId, storeId)`.
Staleness: a snapshot is stale when `now - fetched_at > 24h + jitter_secs` — the
**same rule** the pipeline uses in `PricePayload::is_stale` (`prices/mod.rs`). The
reader honors the payload's `jitter_secs` so both sides agree exactly. On a Redis
miss the API falls back to the latest `price_history` row in Postgres and marks
the listing stale.

### Contract test
`gear-nest-api/.../pricing/PricingRedisContractTest.java` seeds a price in this
exact format and asserts the API reads it live. Keep it green.

---

## Session budget (ADR-003, SPEC §14)

| Key | Type | Value | TTL |
|-----|------|-------|-----|
| `session:{session_id}:questions_remaining` | string (int) | `5` → `0` | 2h |
| `session:{session_id}:inflight` | string | `1` | 90s |

Reserve-then-commit protocol owned by `SessionBudgetService`. Documented here for
completeness; only the API touches these keys (no pipeline writer).

---

## Last-updated marker

| Key | Type | Value |
|-----|------|-------|
| `prices:last_updated` | string | RFC3339 UTC timestamp of the last full price sync. Written by the pipeline after a sync completes. |
