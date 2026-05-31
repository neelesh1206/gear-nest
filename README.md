# GearNest

> Semantic product discovery and price comparison for outdoor, hiking, running, camping, and fitness gear.

Aggregates 50,000+ products across 8 retailers (Amazon, REI, Backcountry, Cabela's, Moosejaw, Steep & Cheap, CampSaver, Garage Grown Gear) with unified search, best-value price ranking, and product-scoped RAG chat grounded in real specs and community reviews.

See [`SPEC.md`](./SPEC.md) for the full technical specification.

---

## Monorepo Layout

```
gear-nest/
├── docs/                   # ARCHITECTURE, SETUP, RUNBOOK, DEPLOYMENT, adr/
├── supabase/migrations/    # SQL schema (owned by Session 0)
├── docker-compose.yml      # Local Postgres + Redis + service stubs
├── gear-nest-pipeline/     # Rust ingestion pipeline  (Session A)
├── gear-nest-api/          # Java 21 + Spring Boot 3  (Session B)
└── gear-nest-web/          # Next.js 16 frontend      (Session C)
```

Parallel implementation guide: [`SPEC.md` §19](./SPEC.md#19-parallel-implementation-guide-claude-code-cli).

---

## Quick Start (Local)

**One command brings up the whole stack with seeded demo data:**

```bash
./scripts/local_demo.sh
```

This boots Postgres + Redis + the Spring Boot API in Docker (Colima),
seeds 5 outdoor products with cross-store listings + live Redis prices,
then starts the Next.js web app on the host. When it prints `Ready.`,
open **http://localhost:3000**.

Try the price-comparison view — each product has 3–4 store quotes ranked
by `BestValueScorer`:

- `/products/osprey-atmos-ag-65` (4 stores)
- `/products/black-diamond-spot-400` (4 stores)
- `/products/msr-pocketrocket-2` (3 stores)

Tear down: `./scripts/local_demo.sh down`.

**Requirements:** Docker (we use [Colima](https://github.com/abiosoft/colima)
on macOS to avoid Docker Desktop licensing), and Node 20+ (`npm install`
once in `gear-nest-web/`).

**Just the infra (no seed, no API):**

```bash
docker compose up -d postgres redis
```

The schema in `supabase/migrations/0001_initial_schema.sql` is applied automatically on first boot.

### Validating across services

The end-to-end live smoke test (multi-store ingest → cross-store entity
resolution → Redis price comparison → API `/prices`) is a separate manual
harness:

```bash
./scripts/phase2_smoke.sh
```

Manual, live, and not a CI gate (it hits real retailer sites). CI keeps
gating on per-service deterministic tests; this harness is the by-hand
check that cross-service seams actually connect on real data.

---

## Pipeline

Rust ingestion pipeline. Scrapes 8 stores, normalizes products, chunks reviews and specs, generates 384-dim embeddings via HuggingFace, bulk-inserts into Postgres + pgvector.

```bash
cd gear-nest-pipeline
cargo run -- --help
cargo run -- migrate                              # apply migrations + partition DDL
cargo run -- scrape-amazon B0XXXXXXXX B0YYYYYY    # PA-API → normalize → embed → DB
cargo run -- ensure-partitions                    # idempotent monthly partitions
cargo test  -- --ignored                          # end-to-end (needs Postgres + Redis)
```

See [`gear-nest-pipeline/CLAUDE.md`](./gear-nest-pipeline/CLAUDE.md).

---

## API

Java 21 + Spring Boot 3 RAG orchestrator. Hybrid search (pgvector + FTS), stratified RAG with MMR, SSE chat streaming, Redis session budget with reserve-then-commit.

```bash
cd gear-nest-api
./mvnw spring-boot:run
```

See [`gear-nest-api/CLAUDE.md`](./gear-nest-api/CLAUDE.md).

---

## Web

Next.js 16 (App Router) + TypeScript strict + Tailwind v4 + shadcn/ui. Catalog, product detail, price comparison, chat panel.

```bash
cd gear-nest-web
npm install && npm run dev
```

See [`gear-nest-web/CLAUDE.md`](./gear-nest-web/CLAUDE.md).

---

## Deployment

GCP hybrid: Cloud Run (API) + Cloud SQL (pgvector) + Vercel (web) + Upstash (Redis). $30/month hard budget cap enforced via Pub/Sub auto-disable. See `docs/DEPLOYMENT.md` and `SPEC.md` §15.
