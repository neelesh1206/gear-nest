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
- **2026-05-27 · contract · claude-pr-review** — GitHub Actions workflow `.github/workflows/claude-pr-review.yml` invokes `anthropics/claude-code-action@v1` on pull_request open/sync/reopen. Prompt enforces scope boundaries, OpenAPI/schema contract conformance, ADR compliance (1-12), and CHANGELOG discipline. Auto-merge intentionally not enabled — manual merge after review.
- **2026-05-27 · contract · claude-pr-review-auth-swap** — Switched workflow auth from `anthropic_api_key` to `claude_code_oauth_token` to use Claude Max subscription quota instead of API credits. Model dropped to `claude-haiku-4-5`. Requires Claude Code GitHub App installed on the repo (https://github.com/apps/claude).
- **2026-05-27 · contract · claude-pr-review-opus** — Upgraded model to `claude-opus-4-7` for review (Max quota covers it). Workflow comment documents Haiku as the model to use if reverting to API key auth.
- **2026-05-27 · contract · claude-pr-review-sticky-comment** — Fix: agent mode wasn't posting reviews. Added `use_sticky_comment: "true"` + `track_progress: "false"` so Claude publishes the verdict as a single consolidated comment. Capped `--max-turns 8` and tightened prompt to ~50% original token count to cap per-PR cost (prior run: 25 turns, $1.20 in Max-equivalent compute, no posted comment).
- **2026-05-27 · contract · claude-pr-review-turn-budget** — Bumped `--max-turns` from 8 to 20 after pipeline PR (large Rust diff) hit `Reached maximum number of turns (8)`. Added explicit turn-budget allocation in prompt instructing Claude to prioritize BLOCKING checks over code-quality deep-reads and ship the verdict by turn 19.
- **2026-05-27 · contract · claude-pr-review-uncap** — Removed `--max-turns` cap entirely and stripped the turn-budget block from the prompt. On Claude Max subscription, quota throttling is the only ceiling — revisit caps only if Max limits are hit during heavy multi-session days.
- **2026-05-27 · contract · claude-pr-review-allowed-tools** — Root cause for "no posted comment": `--allowedTools` was unset, so Claude's `gh pr comment` calls were sandboxed (6 permission denials per run). Added explicit allowlist for `gh pr comment/diff/view/files`, git read commands, and Read/Glob/Grep. Prompt now explicitly instructs Claude to post via `gh pr comment` and to NOT submit the review as its final agent message.
- **2026-05-27 · pipeline · cargo-workspace** — `Cargo.toml` + `rust-toolchain.toml` (1.83 stable) pinning tokio/sqlx/reqwest/redis with rustls; pedantic clippy + forbid unsafe.
- **2026-05-27 · pipeline · db-pool-migrations** — sqlx Postgres pool (max 8) + lightweight runtime migration runner with SHA-256 checksum drift detection, reading `supabase/migrations/`.
- **2026-05-27 · pipeline · amazon-paapi-scraper** — `StoreCrawler` trait + Amazon PA-API 5.0 `GetItems` client with AWS SigV4 signing and 10-ASIN auto-chunking; raw payloads archived to `_gn_scrape_audit`.
- **2026-05-27 · pipeline · normalizer** — title/brand/category canonicalization: ~40-brand alias table, digit-bearing model-token regex, sentence-aware spec chunker, breadcrumb→canonical category projector.
- **2026-05-27 · pipeline · entity-resolution-3tier** — Tier 1 GTIN/ASIN identity, Tier 2 `<brand>:<model>` canonical key, Tier 3 cosine-similarity skeleton with 0.92/0.80 confidence cutoffs (ADR-007).
- **2026-05-27 · pipeline · embeddings** — HuggingFace Inference API client (bge-small-en-v1.5, 384d) with 32-input auto-batching + multi-row pgvector bulk insert; sentence-boundary spec chunks and 256/32 fixed-overlap review chunks.
- **2026-05-27 · pipeline · redis-price-swr** — stale-while-revalidate Redis hash writer with first-write 0–60 min jitter persisted in payload (ADR-009); no key TTL.
- **2026-05-27 · pipeline · price-history-partitions** — idempotent monthly partition DDL at startup (current ±1, +2), batched append, and latest-per-listing fallback path (ADR-010).
- **2026-05-27 · pipeline · integration-test-amazon-50** — end-to-end wiremock-driven test scraping 50 PA-API fixtures through normalize → resolve → embed → DB, asserting audit/listing/product/spec-chunk counts.
- **2026-05-27 · api · spring-boot-scaffold** — `pom.xml` (Spring Boot 3.3.5, Java 21), `application.yml` with `spring.threads.virtual.enabled=true`, Maven wrapper, Dockerfile, package layout under `io.gearnest.api` (config, product, search, pricing, rag, session, embedding, error).
- **2026-05-27 · api · product-controller** — `GET /api/v1/products/search` and `GET /api/v1/products/{slug}` matching `docs/api/openapi.yaml`; DTO records mirror schema component names.
- **2026-05-27 · api · search-service** — `SearchService` + `ProductRepository.hybridSearch` combining pgvector cosine (`<=>`) at 0.6 weight with PostgreSQL FTS `ts_rank` at 0.4 weight, per ADR-005 / SPEC §8.5.
- **2026-05-27 · api · pricing-service** — `PricingService` reads `price:listing:{id}` Redis hash for live price/stock, falls back to latest `price_history` row, sets `isStale=true` when fallback is used or Redis snapshot is older than 25h (ADR-009).
- **2026-05-27 · api · best-value-scorer** — `BestValueScorer` ranks listings by `0.4×price_score + 0.6×rating_score` with 0.02 tiebreaker by price ascending; marks top listing `isBestValue=true`; out-of-stock sorted last (SPEC §12).
- **2026-05-27 · api · session-budget-service** — Redis WATCH/MULTI reserve-then-commit (`DECR questions_remaining` + `SET inflight EX 90`), `commit()` clears inflight on first token, `rollback()` restores budget on HF first-token timeout (SPEC §14).
- **2026-05-27 · api · rag-controller** — `GET /api/v1/chat` SSE endpoint; `gn_session` cookie issued if absent; streaming runs on `Thread.ofVirtual()`; events `token` / `done` / `limit_reached` / `error` match OpenAPI.
- **2026-05-27 · api · rag-service** — Stratified retrieval (5 MMR + 3 negative + 2 spec) over product-scoped pgvector KNN, MMR λ=0.7, HuggingFace chat client with 30s first-token timeout → budget rollback (SPEC §11 + ADR-008).
- **2026-05-27 · api · chat-integration-test** — `ChatStreamIntegrationTest` seeds a product + spec/review chunks in Testcontainers `pgvector/pgvector:pg16`, hits `GET /api/v1/chat`, asserts SSE body contains `event:token`, `event:done`, and the seeded product name.
