# ADR-009: Stale-while-revalidate for Redis price cache (no hard TTL)

**Status:** Accepted
**Date:** 2026-05-27

## Decision
Redis price keys are permanent (no EXPIRE). Staleness is application-managed via an embedded `fetched_at` timestamp. Reads serve stale data immediately and trigger async background refresh when data is > 24 hours old. First-write jitter (0–60 min) spreads the 24-hour staleness window across the catalog.

## Rationale
Hard TTL on 50,000 price keys set by a single batch run creates synchronized expiry — a thundering herd that shifts all price reads onto Postgres simultaneously under morning traffic. Stale-while-revalidate keeps Postgres isolated from Redis misses regardless of pipeline timing. Stale price display is bounded at 25 hours and is shown honestly in the UI.

## Trade-off
Application must manage staleness rather than relying on Redis TTL semantics. Slightly more complex PricingService logic. Accepted — the thundering herd risk is a real production failure mode.
