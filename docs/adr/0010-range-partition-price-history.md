# ADR-010: Range-partition price_history by month

**Status:** Accepted
**Date:** 2026-05-27

## Decision
`price_history` is range-partitioned on `fetched_at` with monthly child partitions created dynamically by the Rust pipeline.

## Rationale
400k rows/day × 365 days = 146M rows/year. Without partitioning, the B-tree index on `(listing_id, fetched_at DESC)` grows to 100M+ entries, degrading both write performance (daily price sync) and read performance (trend chart queries). Monthly partitioning ensures daily writes target the hot current partition; trend queries hit ≤ 2 partitions; retention policy = `DROP PARTITION` (O(1), no lock). Directly maps to the list-partitioned Postgres pattern from PRISM / cxt-msg-asset-service at Walmart.

## Trade-off
Foreign key enforcement is at the application layer (partitioned tables can't have FK with non-partitioned parents). Pipeline must create next month's partition before month rollover — handled by idempotent DDL at pipeline startup.
