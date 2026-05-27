# ADR-006: Redis for mutable price data (MVCC decoupling)

**Status:** Accepted
**Date:** 2026-05-27

## Decision
Current prices and in-stock flags live in Redis hashes (`prices:{product_id}`), not as columns on `store_listings`.

## Rationale
PostgreSQL MVCC writes a new row version on every UPDATE. Daily price sync across 50,000 listings = 50,000 UPDATE statements = severe table bloat and forced vacuum cycles that degrade frontend read latency. Redis hash (`HSET "prices:{product_id}" store_id {price, stock, ts}`) is append-friendly by nature — no versioning overhead. `price_history` in Postgres remains the append-only source of truth for trends.

## Trade-off
Price reads require two-source lookup (Postgres + Redis). Handled transparently in `PricingService` with Redis-miss fallback to `price_history`.
