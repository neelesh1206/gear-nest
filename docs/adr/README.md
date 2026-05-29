# Architecture Decision Records

Format: `NNNN-kebab-case-title.md`. ADRs are append-only — once accepted, do not edit; supersede with a new ADR.

## Number Reservations

To prevent renumber conflicts across parallel Claude sessions, ADR numbers are pre-allocated by track:

| Range | Track | Owner |
|-------|-------|-------|
| 001–012 | Foundation | Session 0 (transcribed from SPEC §18) |
| 013–015 | Pipeline | Session A |
| 016–018 | API | Session B |
| 019–020 | Web | Session C |
| 021+ | Contract / ongoing | Session 0 |

Sessions claim a reserved number and fill in content. They do **not** renumber.

## Index

- [0001 — pgvector over a dedicated vector database](./0001-pgvector-over-dedicated-vector-db.md)
- [0002 — Rust for ingestion, not Python](./0002-rust-for-ingestion-not-python.md)
- [0003 — Session-based LLM limits without auth](./0003-session-based-llm-limits-without-auth.md)
- [0004 — BAAI/bge-small-en-v1.5 for embeddings](./0004-bge-small-embeddings.md)
- [0005 — Hybrid search (vector + FTS) for catalog](./0005-hybrid-search-vector-plus-fts.md)
- [0006 — Redis for mutable price data (MVCC decoupling)](./0006-redis-for-mutable-price-data.md)
- [0007 — Three-tier entity resolution with confidence gating](./0007-three-tier-entity-resolution.md)
- [0008 — Stratified + MMR retrieval over pure top-K cosine](./0008-stratified-mmr-retrieval.md)
- [0009 — Stale-while-revalidate for Redis price cache (no hard TTL)](./0009-stale-while-revalidate-price-cache.md)
- [0010 — Range-partition price_history by month](./0010-range-partition-price-history.md)
- [0011 — MinHash Stage 2 gated at 150-char review body length](./0011-minhash-150-char-threshold.md)
- [0012 — GCP (Cloud Run + Cloud SQL) over Fly.io](./0012-gcp-over-flyio.md)
- [0013 — Extend StoreCrawler for discovery + a tiered transport abstraction](./0013-storecrawler-transport-tiers.md)
- [0014 — Proxy tier delegates IP rotation to the provider](./0014-proxy-tier-residential-rotation.md)
- [0015 — Headless tier: one chromiumoxide browser with a tab pool](./0015-headless-browser-pool.md)
- [0016 — *reserved (API)*](./0016-reserved-api.md)
- [0017 — *reserved (API)*](./0017-reserved-api.md)
- [0018 — *reserved (API)*](./0018-reserved-api.md)
- [0019 — *reserved (Web)*](./0019-reserved-web.md)
- [0020 — *reserved (Web)*](./0020-reserved-web.md)
- [0021 — Redis price schema is a pinned cross-service contract](./0021-redis-schema-contract.md)
- [0022 — External scheduling (Cloud Scheduler → one-shot Cloud Run Job) over in-process cron](./0022-external-scheduling-over-in-process-cron.md)
