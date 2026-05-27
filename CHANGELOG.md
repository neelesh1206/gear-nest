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
