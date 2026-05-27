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

```bash
docker compose up -d postgres redis
```

The schema in `supabase/migrations/0001_initial_schema.sql` is applied automatically on first boot.

---

## Pipeline

Rust ingestion pipeline. Scrapes 8 stores, normalizes products, chunks reviews and specs, generates 384-dim embeddings via HuggingFace, bulk-inserts into Postgres + pgvector.

```bash
cd gear-nest-pipeline
cargo run -- --help
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
