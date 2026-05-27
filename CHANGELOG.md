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
