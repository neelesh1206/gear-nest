@../AGENTS.md

# GearNest Pipeline — Session A Scope Boundaries

## YOU OWN
- Everything under `gear-nest-pipeline/` (Rust crate)
- May READ: `supabase/migrations/` (schema reference), `docs/api/` (for types only)
- May READ: root `docker-compose.yml` (to start Postgres + Redis)

## DO NOT TOUCH
- `gear-nest-api/` — any file, any reason
- `gear-nest-web/` — any file, any reason
- `supabase/migrations/` — read-only; new migrations go to Session 0 for review
- `docs/adr/` — claim your pre-allocated ADR number (ADR-013 to ADR-015); do not renumber

## CONFLICT ZONES
- `CHANGELOG.md` — append-only; add one line at the bottom, never rewrite history
- `README.md` — append to the Pipeline section only; leave API/Web sections untouched

## Coding conventions
- Rust edition 2021, stable toolchain (see `rust-toolchain.toml`)
- Never hardcode DB credentials; read `DATABASE_URL` from environment
- `cargo fmt` + `cargo clippy -- -D warnings` must pass before commit
- No comments unless WHY is non-obvious

## Running the stack
```bash
docker compose up -d postgres redis
cargo run -- --help
```

## Phase 1 Track (SPEC §19.8)
- [ ] Rust workspace: `Cargo.toml`, `rust-toolchain.toml`, workspace members
- [ ] `src/db/` — Postgres connection pool (`sqlx`), migration runner
- [ ] `src/scrapers/amazon.rs` — PA API client, product fetch, raw JSON storage
- [ ] `src/normalizer/` — title normalization, category mapping, brand alias table
- [ ] `src/entity_resolution/` — Tier 1 (GTIN/ASIN), Tier 2 (structured), Tier 3 skeleton
- [ ] `src/embeddings/` — HuggingFace Inference API client, batch embed, pgvector insert
- [ ] `src/prices/` — stale-while-revalidate Redis writer with jitter
- [ ] `src/price_history/` — partition-aware append, idempotent DDL at startup
- [ ] Integration test: scrape 50 Amazon products → normalize → embed → assert in DB
