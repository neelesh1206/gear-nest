# Changelog

Format: `YYYY-MM-DD · <service> · <feature> — one-line summary`

Append only. Never rewrite or reorder. Per-service section ownership:
- `pipeline` — Session A
- `api` — Session B
- `web` — Session C
- `contract` — Session 0 (schema, OpenAPI, infra, docs)

---

## 2026-05-27 — Phase 1 (in progress)

- **2026-05-27 · contract · session-0-bootstrap** — Monorepo scaffold: root README, CHANGELOG, CLAUDE.md, AGENTS.md, .gitignore, .env.example. Service directories created as empty.
- **2026-05-27 · contract · postgres-schema** — Initial schema in `supabase/migrations/0001_initial_schema.sql`: products, store_listings, price_history (monthly range partitions for 2026-05/06/07), reviews, review_chunks + spec_chunks (btree on product_id, no HNSW per ADR-001), ai_summaries, stores. pgvector + pg_trgm + pgcrypto extensions enabled. Seeded 8 stores.
- **2026-05-27 · contract · docker-compose** — Postgres 16+pgvector with auto-applied migrations, Redis 7-alpine with healthchecks, profiled service stubs for pipeline/api/web.
- **2026-05-27 · contract · openapi-spec** — `docs/api/openapi.yaml` covering all Phase 1 endpoints (search, detail, prices, reviews, summary, chat SSE).
- **2026-05-27 · contract · service-claude-md** — Per-service CLAUDE.md scope files for pipeline, api, web (SPEC §19.6).
- **2026-05-27 · contract · adr-stubs** — ADR-001 through ADR-012 transcribed from SPEC §18; ADR-013 through ADR-020 reserved as empty stubs (Pipeline 013-015, API 016-018, Web 019-020).
- **2026-05-27 · pipeline · cargo-workspace** — `Cargo.toml` + `rust-toolchain.toml` (1.83 stable) pinning tokio/sqlx/reqwest/redis with rustls; pedantic clippy + forbid unsafe.
- **2026-05-27 · pipeline · db-pool-migrations** — sqlx Postgres pool (max 8) + lightweight runtime migration runner with SHA-256 checksum drift detection, reading `supabase/migrations/`.
- **2026-05-27 · pipeline · amazon-paapi-scraper** — `StoreCrawler` trait + Amazon PA-API 5.0 `GetItems` client with AWS SigV4 signing and 10-ASIN auto-chunking; raw payloads archived to `_gn_scrape_audit`.
- **2026-05-27 · pipeline · normalizer** — title/brand/category canonicalization: ~40-brand alias table, digit-bearing model-token regex, sentence-aware spec chunker, breadcrumb→canonical category projector.
- **2026-05-27 · pipeline · entity-resolution-3tier** — Tier 1 GTIN/ASIN identity, Tier 2 `<brand>:<model>` canonical key, Tier 3 cosine-similarity skeleton with 0.92/0.80 confidence cutoffs (ADR-007).
- **2026-05-27 · pipeline · embeddings** — HuggingFace Inference API client (bge-small-en-v1.5, 384d) with 32-input auto-batching + multi-row pgvector bulk insert; sentence-boundary spec chunks and 256/32 fixed-overlap review chunks.
- **2026-05-27 · pipeline · redis-price-swr** — stale-while-revalidate Redis hash writer with first-write 0–60 min jitter persisted in payload (ADR-009); no key TTL.
- **2026-05-27 · pipeline · price-history-partitions** — idempotent monthly partition DDL at startup (current ±1, +2), batched append, and latest-per-listing fallback path (ADR-010).
- **2026-05-27 · pipeline · integration-test-amazon-50** — end-to-end wiremock-driven test scraping 50 PA-API fixtures through normalize → resolve → embed → DB, asserting audit/listing/product/spec-chunk counts.
