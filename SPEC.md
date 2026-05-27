# GearNest — Technical Specification

> Semantic product discovery and price comparison for outdoor, hiking, running, camping, and fitness gear.
> Built by an outdoor enthusiast for the community.

**Version:** 1.3  
**Date:** 2026-05-27  
**Author:** Neelesh Kakaraparthi  
**Status:** Pre-build  
**Changelog v1.1:** Fixed entity resolution strategy (3-tier matching replaces string similarity); removed HNSW anti-pattern from chunk tables (exact KNN on filtered subset); added anti-bot scraping tiers; decoupled mutable prices to Redis (MVCC fix); replaced naive Jaccard dedup with MinHash LSH; added stratified + MMR retrieval; added reserve-then-commit for session tokens.  
**Changelog v1.2:** Corrected Next.js version (16.x), Tailwind version (v4); updated portfolio integration to TypeScript data object (not MDX); added monorepo layout, docs/ structure, CLAUDE.md/AGENTS.md requirements, CHANGELOG format — all aligned to MarketMind conventions.  
**Changelog v1.3:** Replaced Fly.io with GCP hybrid (Cloud Run + Cloud SQL + Upstash Redis); added IAM/Workload Identity setup, Secret Manager, VPC private networking, GitHub Actions CI/CD, $30/month billing budget with Pub/Sub auto-disable Cloud Function; added ADR-012; added §19 Parallel Implementation Guide for Claude Code CLI sessions.

---

## Table of Contents

1. [Vision & Goals](#1-vision--goals)
2. [Project Name & Branding](#2-project-name--branding)
3. [System Architecture Overview](#3-system-architecture-overview)
4. [Technology Stack](#4-technology-stack)
5. [Data Sources & Coverage](#5-data-sources--coverage)
6. [Data Model](#6-data-model)
7. [Rust Ingestion Pipeline](#7-rust-ingestion-pipeline)
8. [Java RAG Orchestrator (Spring Boot)](#8-java-rag-orchestrator-spring-boot)
9. [Next.js Frontend](#9-nextjs-frontend)
10. [API Contracts](#10-api-contracts)
11. [RAG System Design](#11-rag-system-design)
12. [Price Comparison Logic](#12-price-comparison-logic)
13. [Review Aggregation Strategy](#13-review-aggregation-strategy)
14. [Session Management & LLM Limits](#14-session-management--llm-limits)
15. [Deployment Architecture](#15-deployment-architecture)
16. [Build Phases](#16-build-phases)
17. [Portfolio Integration](#17-portfolio-integration)
18. [Key Architecture Decisions (ADRs)](#18-key-architecture-decisions-adrs)
19. [Parallel Implementation Guide (Claude Code CLI)](#19-parallel-implementation-guide-claude-code-cli)

---

## 1. Vision & Goals

### Problem Statement

Outdoor and fitness enthusiasts shop across 8+ specialized retailers (REI, Backcountry, Cabela's, etc.) comparing prices manually, reading scattered reviews, and relying on keyword search that fails for nuanced queries like *"which sleeping bag works below -20°F and packs under 2 lbs?"*

### Product Vision

GearNest is a centralized outdoor + fitness gear aggregator where users can:
- **Discover** 50,000+ products across 8 retailers in one place
- **Compare** live-ish prices with a "best value" ranking (price + rating)
- **Ask** complex natural-language questions and get synthesized answers from real product specs and community reviews
- **Trust** reviews aggregated from multiple verified sources with authentic samples per rating tier

### Non-Goals (v1)

- No user accounts or wishlists
- No transaction processing — GearNest links out; it does not sell
- No mobile app
- No real-time inventory (in-stock status is best-effort)

---

## 2. Project Name & Branding

**Name:** GearNest  
**Tagline:** *Find your gear. Ask anything.*  
**Domain target:** gearnest.io or gearnest.app  
**Logo concept:** Minimalist nest/web icon combining a mountain silhouette

**Why "GearNest":** A nest is a hub — everything gathered in one place. Evokes community (outdoor enthusiasts sharing knowledge) without being generic like "OutdoorSearch."

---

## 3. System Architecture Overview

```
┌─────────────────────────────────────────────────────────────────┐
│                        BATCH LAYER (Rust)                       │
│                                                                 │
│  ┌──────────────┐  ┌──────────────┐  ┌─────────────────────┐   │
│  │ Store        │  │  Normalizer  │  │  Embedding          │   │
│  │ Scrapers     │→ │  + Chunker   │→ │  Generator          │   │
│  │ (8 stores)   │  │              │  │  (HuggingFace API)  │   │
│  └──────────────┘  └──────────────┘  └─────────────────────┘   │
│                                               ↓                  │
└───────────────────────────────────────────────┼─────────────────┘
                                                │ bulk insert
                                                ↓
                        ┌───────────────────────────────┐
                        │   PostgreSQL 16 + pgvector    │
                        │   Products | Reviews | Prices │
                        │   + Vector embeddings (384d)  │
                        └───────────────────────────────┘
                                        ↑ ↓
                        ┌───────────────────────────────┐
                        │  Java 21 / Spring Boot 3      │
                        │  RAG Orchestrator + REST API  │
                        │  Spring AI | Redis Cache      │
                        └───────────────────────────────┘
                                        ↑ SSE / REST
                        ┌───────────────────────────────┐
                        │   Next.js 15 Frontend         │
                        │   Product Catalog | Chat UI   │
                        │   Price Comparison | Reviews  │
                        └───────────────────────────────┘
```

### Service Responsibilities

| Service | Language | Responsibility |
|---------|----------|----------------|
| Ingestion Pipeline | Rust | Scrape, normalize, chunk, embed, bulk-insert |
| RAG Orchestrator | Java 21 + Spring Boot 3 | REST API, RAG queries, SSE streaming, business logic |
| Frontend | Next.js 15 + TypeScript | UI, SSE consumer, product/search/chat pages |
| Vector + Relational Store | PostgreSQL + pgvector | All persistent data and similarity search |
| Cache | Redis | Price cache, session token budgets, AI summary cache |

---

## 4. Technology Stack

### Rust (Ingestion Pipeline)

| Crate | Purpose |
|-------|---------|
| `tokio` | Async runtime |
| `reqwest` | HTTP client — scraping + HuggingFace Inference API |
| `scraper` | HTML parsing with CSS selectors |
| `serde` / `serde_json` | JSON serialization |
| `sqlx` | Async PostgreSQL driver with compile-time query checks |
| `tiktoken-rs` | Token counting for chunking strategy |
| `rayon` | CPU-bound parallelism for normalization |
| `tracing` / `tracing-subscriber` | Structured logging |
| `anyhow` | Error handling |
| `tokio-cron-scheduler` | Cron jobs for daily price sync and weekly full sync |
| `governor` | Rate limiting per store (respect crawl delays) |

### Java (RAG Orchestrator)

| Dependency | Purpose |
|-----------|---------|
| Spring Boot 3.3 | Web framework |
| Spring AI 1.x | HuggingFace + LLM integration, embedding abstractions |
| Spring Data JPA | ORM + pgvector query support |
| Spring Web (SSE) | `SseEmitter` for streaming LLM responses |
| Spring Cache | Redis-backed caching annotations |
| Spring Data Redis | Session token budget tracking |
| `pgvector-spring` | Vector column support in Spring Data |
| Lombok | Boilerplate reduction |
| MapStruct | DTO ↔ entity mapping |

### Frontend

| Technology | Purpose |
|-----------|---------|
| Next.js 16 App Router | Framework (matches MarketMind + portfolio versions) |
| React 19 | UI |
| TypeScript strict | Type safety, no `any`, no `@ts-ignore` |
| Tailwind CSS v4 | Styling — dark mode via `dark:` prefix + `@custom-variant dark` in globals.css |
| shadcn/ui | Component system |
| Tanstack Query | Data fetching + cache |
| EventSource API | SSE consumer for chat streaming |
| next-themes | Dark/light mode |
| Framer Motion | Micro-animations |

### Infrastructure

| Component | Technology |
|----------|-----------|
| Database | PostgreSQL 16 + pgvector extension |
| Cache | Redis 7 |
| Containerization | Docker + Docker Compose |
| Production hosting | Fly.io (API + DB) + Vercel (frontend) |
| CI/CD | GitHub Actions |
| Embeddings | HuggingFace Inference API — `BAAI/bge-small-en-v1.5` (384d) |
| LLM | HuggingFace Inference API — `mistralai/Mistral-7B-Instruct-v0.3` |

---

## 5. Data Sources & Coverage

### Stores (v1 — 8 stores)

| Store | Data Method | Notes |
|-------|------------|-------|
| Amazon | Product Advertising API (affiliate) | Best structured data, verified reviews available |
| REI | CJ Affiliate API + scrape supplement | Affiliate program via Commission Junction |
| Backcountry | Scrape (no public API) | Rich user reviews, good specs |
| Cabela's / Bass Pro Shops | Scrape | Same parent company (Vista Outdoor), merged catalog |
| Moosejaw | Scrape | Strong review culture, niche brands |
| Steep & Cheap | Scrape | Deals/sale focus — price signal important |
| CampSaver | Scrape | Strong ultralight backpacking selection |
| Garage Grown Gear | Scrape | Cottage industry brands not on major stores |

### Product Categories

```
Outdoor & Trail
├── Hiking Footwear (boots, trail runners, sandals)
├── Trekking & Navigation (poles, GPS, maps)
├── Camping Shelter (tents, tarps, bivy)
├── Sleeping Systems (bags, pads, quilts)
├── Backpacks & Bags (packs, daypacks, stuff sacks)
├── Camp Kitchen (stoves, cookware, water treatment)
├── Technical Apparel (base, mid, shell, rain)
├── Climbing (harnesses, helmets, protection — light gear only)
└── Hydration & Nutrition

Running
├── Road Running Shoes
├── Trail Running Shoes
├── Running Apparel
└── Hydration Vests & Belts

Fitness & CrossFit
├── Barbells & Plates
├── Kettlebells & Dumbbells
├── Gymnastics (rings, pull-up bars)
├── Conditioning (jump ropes, sleds, bands)
└── Recovery (foam rollers, massage guns)
```

### Target Scale

| Metric | Target |
|--------|--------|
| Total products | 50,000 |
| Total reviews indexed | ~2,000,000 |
| Total vector chunks | ~8,000,000 |
| Stores | 8 |
| Price refresh cadence | Daily at 06:00 UTC |
| Full product sync | Weekly Sunday 02:00 UTC |

---

## 6. Data Model

### Core Tables

```sql
-- Canonical product record (store-agnostic)
CREATE TABLE products (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug            TEXT UNIQUE NOT NULL,           -- url-safe name
    name            TEXT NOT NULL,
    brand           TEXT NOT NULL,
    category        TEXT NOT NULL,
    subcategory     TEXT,
    description     TEXT,
    specs           JSONB,                          -- weight, dimensions, materials, etc.
    primary_image   TEXT,
    created_at      TIMESTAMPTZ DEFAULT NOW(),
    updated_at      TIMESTAMPTZ DEFAULT NOW()
);

-- One row per product per store (static metadata only — no price/stock here; see Redis)
CREATE TABLE store_listings (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id          UUID REFERENCES products(id) ON DELETE CASCADE,
    store_id            TEXT NOT NULL,              -- 'amazon', 'rei', 'backcountry', etc.
    store_product_id    TEXT NOT NULL,              -- store's internal ID/SKU
    store_url           TEXT NOT NULL,
    affiliate_url       TEXT,                       -- affiliate link where available
    store_rating        NUMERIC(3,2),               -- store-specific average rating
    store_review_count  INT DEFAULT 0,
    match_confidence    TEXT NOT NULL DEFAULT 'EXACT'
                        CHECK (match_confidence IN ('EXACT','HIGH','MEDIUM','CANDIDATE')),
    last_synced_at      TIMESTAMPTZ,
    UNIQUE(store_id, store_product_id)
);
-- CANDIDATE rows are not shown in UI — they await manual/automated review

-- Current prices live in Redis: "prices:{product_id}" hash
-- Mutable price/stock kept out of Postgres to avoid MVCC write amplification

-- Price history — range-partitioned by month on fetched_at
-- 400k rows/day × 365 = 146M rows/year. Partitioning keeps each month's
-- index small enough to stay resident in buffer pool. Trend queries
-- (last 30 days) hit at most 2 partitions. Retention = DROP PARTITION (O(1)).
-- Mirrors Neelesh's list-partitioned Postgres from PRISM / cxt-msg-asset-service.
CREATE TABLE price_history (
    id          BIGSERIAL,
    listing_id  UUID NOT NULL,              -- FK enforced at app layer (partitioned tables
    price       NUMERIC(10,2) NOT NULL,     -- can't have FK to non-partitioned parent easily)
    in_stock    BOOLEAN,
    fetched_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id, fetched_at)            -- partition key must be in PK for range partition
) PARTITION BY RANGE (fetched_at);

-- Create monthly partitions (Rust pipeline creates next month's partition
-- at the end of each run if it doesn't exist yet — idempotent DDL)
CREATE TABLE price_history_2026_05 PARTITION OF price_history
    FOR VALUES FROM ('2026-05-01') TO ('2026-06-01');
CREATE TABLE price_history_2026_06 PARTITION OF price_history
    FOR VALUES FROM ('2026-06-01') TO ('2026-07-01');
-- ... created dynamically by pipeline

-- Index per partition (auto-inherited by child partitions in PG16)
CREATE INDEX ON price_history (listing_id, fetched_at DESC);

-- Retention: DROP TABLE price_history_2025_05 — instant, no DELETE locks
-- Default retention policy: keep 13 months (12 months display + 1 overlap)

-- Individual reviews scraped per store
CREATE TABLE reviews (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id          UUID REFERENCES products(id) ON DELETE CASCADE,
    store_id            TEXT NOT NULL,
    source_review_id    TEXT,                       -- store's review ID for dedup
    reviewer_id_hash    TEXT,                       -- hashed reviewer identifier
    rating              SMALLINT NOT NULL CHECK (rating BETWEEN 1 AND 5),
    title               TEXT,
    body                TEXT NOT NULL,
    verified_purchase   BOOLEAN DEFAULT FALSE,
    helpful_votes       INT DEFAULT 0,
    review_date         DATE,
    scraped_at          TIMESTAMPTZ DEFAULT NOW(),
    UNIQUE(store_id, source_review_id)
);

-- pgvector: review chunks indexed for semantic search
CREATE TABLE review_chunks (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    review_id   UUID REFERENCES reviews(id) ON DELETE CASCADE,
    product_id  UUID REFERENCES products(id) ON DELETE CASCADE,
    chunk_text  TEXT NOT NULL,
    chunk_index SMALLINT NOT NULL,
    embedding   vector(384),
    rating      SMALLINT,                           -- inherited from parent review
    store_id    TEXT
);
-- NO HNSW index here. Chat is always product-scoped: the WHERE product_id = ? filter
-- reduces to ~60-80 rows per product. Exact KNN over that set is microseconds.
-- HNSW on a 99.98%-filtered graph degrades to sequential scan — worse than exact KNN.
CREATE INDEX ON review_chunks (product_id);        -- btree for fast row filtering

-- pgvector: product spec/description chunks
CREATE TABLE spec_chunks (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id  UUID REFERENCES products(id) ON DELETE CASCADE,
    chunk_text  TEXT NOT NULL,
    chunk_index SMALLINT NOT NULL,
    source_type TEXT NOT NULL CHECK (source_type IN ('description', 'specs', 'features')),
    embedding   vector(384)
);
-- Same reasoning: no HNSW. Exact KNN over ~10-15 spec chunks per product is trivially fast.
CREATE INDEX ON spec_chunks (product_id);

-- Cached AI-generated summaries (invalidated on new reviews)
CREATE TABLE ai_summaries (
    product_id      UUID PRIMARY KEY REFERENCES products(id),
    summary_text    TEXT NOT NULL,
    pros            TEXT[],
    cons            TEXT[],
    review_count    INT,                            -- count at time of generation
    generated_at    TIMESTAMPTZ DEFAULT NOW()
);

-- Stores registry
CREATE TABLE stores (
    id              TEXT PRIMARY KEY,               -- 'amazon', 'rei', etc.
    display_name    TEXT NOT NULL,
    base_url        TEXT NOT NULL,
    logo_url        TEXT,
    affiliate_type  TEXT,                           -- 'pa-api', 'cj', 'scrape'
    active          BOOLEAN DEFAULT TRUE
);
```

---

## 7. Rust Ingestion Pipeline

### Project Structure

```
gear-nest-pipeline/
├── Cargo.toml
├── src/
│   ├── main.rs                 # Scheduler + CLI entrypoint
│   ├── config.rs               # Env-based config
│   ├── db/
│   │   ├── mod.rs
│   │   └── loader.rs           # Bulk insert logic (ON CONFLICT DO UPDATE)
│   ├── scrapers/
│   │   ├── mod.rs              # StoreCrawler trait
│   │   ├── amazon.rs           # Amazon PA API
│   │   ├── rei.rs
│   │   ├── backcountry.rs
│   │   ├── cabelas.rs
│   │   ├── moosejaw.rs
│   │   ├── steepandcheap.rs
│   │   ├── campsaver.rs
│   │   └── garagerowngear.rs
│   ├── normalizer/
│   │   ├── mod.rs
│   │   ├── product.rs          # Canonical product mapping
│   │   └── review.rs           # Review dedup + normalize
│   ├── chunker/
│   │   ├── mod.rs
│   │   ├── review_chunker.rs   # Fixed-size with overlap
│   │   └── spec_chunker.rs     # Sentence-boundary chunking
│   ├── embedder/
│   │   └── mod.rs              # HuggingFace Inference API calls (batched)
│   └── jobs/
│       ├── full_sync.rs        # Weekly: all products + reviews
│       ├── price_sync.rs       # Daily: prices only
│       └── review_sync.rs      # Daily: new reviews only
```

### StoreCrawler Trait

```rust
#[async_trait]
pub trait StoreCrawler: Send + Sync {
    fn store_id(&self) -> &'static str;
    async fn crawl_products(&self, category: &Category) -> Result<Vec<RawProduct>>;
    async fn crawl_reviews(&self, product_id: &str, pages: u32) -> Result<Vec<RawReview>>;
    async fn fetch_price(&self, store_product_id: &str) -> Result<PriceUpdate>;
}
```

Each store implements this trait. The scheduler dispatches all stores concurrently using `tokio::spawn` + `JoinSet`.

### Chunking Strategy

**Review chunks** — Fixed-size with overlap (short texts, uniform density):
- Target: 256 tokens per chunk
- Overlap: 32 tokens
- Rationale: Reviews are naturally short (100–400 words). Fixed-size ensures even distribution in vector space.

**Spec/description chunks** — Sentence-boundary aware:
- Target: 300 tokens per chunk, max 500 tokens
- Split at sentence endings, never mid-sentence
- Rationale: Product specs have structured sentences ("Insulation: 800-fill goose down. Weight: 14 oz."). Cutting mid-sentence loses attribute-value pairs.

Both chunk types inherit `product_id` + `store_id` for filtered retrieval.

### Embedding Generation

```rust
// Batched embedding calls — 32 texts per API request
pub async fn embed_batch(
    client: &reqwest::Client,
    texts: Vec<String>,
    api_key: &str,
) -> Result<Vec<Vec<f32>>> {
    let url = "https://api-inference.huggingface.co/models/BAAI/bge-small-en-v1.5";
    // POST with {"inputs": [...32 texts...]}
    // Returns [[f32; 384]; 32]
}
```

**Model:** `BAAI/bge-small-en-v1.5`
- 384 dimensions (compact, fast)
- Outperforms all-MiniLM-L6-v2 on retrieval benchmarks
- ~45ms per batch of 32 on HuggingFace Pro inference

### Entity Resolution (Cross-Store Product Matching)

Matching the same physical product across 8 stores is the hardest correctness problem in the pipeline. Three-tier confidence system:

```rust
pub enum MatchConfidence { Exact, High, Medium, Candidate }

pub fn resolve_product(raw: &RawProduct, existing: &[ProductRecord]) -> MatchResult {
    // Tier 1 — Structural identifiers (O(1), most reliable)
    // Amazon PA API returns ASIN + often GTIN/UPC. REI via CJ returns GTINs.
    if let Some(gtin) = &raw.gtin {
        if let Some(p) = existing.iter().find(|p| p.gtin.as_deref() == Some(gtin)) {
            return MatchResult { product_id: p.id, confidence: Exact };
        }
    }

    // Tier 2 — Structured attribute extraction (O(n), deterministic)
    // Normalize brand aliases + extract model tokens via regex.
    // "Mountain Safety Research Pocket Rocket II Stove" → {brand:"msr", model:"pocketrocket-2"}
    let attrs = extract_attributes(&raw.title, &BRAND_ALIASES, &MODEL_PATTERNS);
    if let Some(p) = existing.iter().find(|p| p.canonical_key == attrs.canonical_key()) {
        return MatchResult { product_id: p.id, confidence: High };
    }

    // Tier 3 — Embedding similarity on normalized representation (fallback)
    // Compare embeddings of canonical attribute strings, not raw titles.
    if let Some((p, score)) = find_nearest_by_embedding(&attrs.normalized_repr(), existing) {
        if score > 0.92 {
            return MatchResult { product_id: p.id, confidence: Medium };
        }
        if score > 0.80 {
            return MatchResult { product_id: p.id, confidence: Candidate };
        }
    }

    // No match — create new canonical product
    MatchResult { product_id: Uuid::new_v4(), confidence: Exact }
}
```

`CANDIDATE` confidence listings are written to the DB but excluded from the UI price comparison table. A separate admin endpoint (`GET /api/admin/candidates`) surfaces them for review. Only `EXACT | HIGH | MEDIUM` rows are shown to users.

### CANDIDATE Recovery Loop (Admin Actions)

Two distinct admin operations exist — their data consequences differ:

**Case A — Confirm match** (this Amazon listing IS product X, just low confidence):
```
POST /api/admin/candidates/{listing_id}/confirm
  → UPDATE store_listings SET match_confidence = 'MEDIUM' WHERE id = ?
  
No backfill needed:
  - price_history rows already exist (pipeline writes regardless of confidence)
  - review_chunks already embedded (embedding pipeline is confidence-agnostic)
  - listing immediately appears in price comparison table on next API read
```

**Case B — Reassign match** (this listing belongs to product Y, not X):
```
POST /api/admin/candidates/{listing_id}/reassign  { target_product_id: Y }
  → UPDATE store_listings SET product_id = Y, match_confidence = 'HIGH' WHERE id = ?
  
  price_history follows automatically — joined via listing_id, no row rewrite
  
  review_chunks need re-pointing (product_id is embedded for retrieval scoping):
  → UPDATE review_chunks SET product_id = Y
    WHERE review_id IN (SELECT id FROM reviews 
                        WHERE product_id = X_old 
                        AND store_id = listing.store_id)
  
  Embedding values do NOT change — text is unchanged, only the FK is re-pointed.
  
  Invalidate AI summaries for both affected products:
  → DELETE FROM ai_summaries WHERE product_id IN (X_old, Y)
  → Enqueue background re-generation for both products
```

Spring Boot admin controller executes Case A synchronously (single UPDATE). Case B runs in a virtual thread — the HTTP response returns immediately with `202 Accepted`; a `@Async` service completes the review_chunk re-association and summary invalidation in the background. The admin UI polls a `/api/admin/jobs/{id}/status` endpoint to confirm completion.

### Price Sync Job (Daily)

```
For each active store_listing (EXACT|HIGH|MEDIUM confidence only):
  1. Fetch current price + in_stock from store (scrape product page or API)
  2. INSERT INTO price_history partition (fetched_at month) — see §6 range partitioning
  3. HSET "prices:{product_id}" store_id {price, in_stock, currency, fetched_at}
     -- NO TTL on the key — stale-while-revalidate pattern (see below)
  4. Rate limit: per-store governor (see anti-bot section)
  5. Retry with exponential backoff on 429/503
```

After full sync, SET `prices:last_updated` to current UTC timestamp.

### Stale-While-Revalidate Price Pattern (Cache-Stampede Prevention)

**Problem with hard TTL:** If the daily sync sets `EXPIRE 90000` on all 50,000 product keys simultaneously, they all expire at roughly 07:00 UTC the next day. Under morning traffic, 50,000 simultaneous cache misses hit Postgres in a thundering herd.

**Solution: No TTL on price keys. Staleness is application-managed.**

```
Java API — PricingService.getPrices(productId):

  1. HGETALL "prices:{product_id}"
     a. Key missing (truly new product) → query price_history, write to Redis, return
     b. Key present, fetched_at < 24h ago → return cached prices (fresh)
     c. Key present, fetched_at >= 24h ago → SERVE STALE immediately to user
        + submit background task to RefreshExecutor (virtual thread pool, non-blocking)
        + background task: fetch from price_history, HSET with new fetched_at
        + next request for this product gets fresh data

  RefreshExecutor is a bounded virtual thread pool (max 50 concurrent).
  Stale prices show "[Updated: yesterday]" in the UI — honest to the user.
  Database is never hit synchronously for a cache-stale condition.
```

**Jitter on first write:** When the Rust pipeline writes a Redis hash for the first time (new product or after a deliberate cache clear), add `0–60 min` random jitter to the embedded `fetched_at` timestamp. This spreads the 24-hour staleness window across the product catalog so no two products become stale at the same second.

```rust
// In price_sync.rs — write with jitter on first insert
let jitter_secs = rand::thread_rng().gen_range(0..3600u64);
let effective_fetched_at = fetched_at - Duration::from_secs(jitter_secs);
redis.hset("prices:{product_id}", [
    ("price", price),
    ("in_stock", in_stock),
    ("fetched_at", effective_fetched_at.timestamp()),
]).await?;
// No EXPIRE call — key is permanent
```

### Anti-Bot Scraping Strategy

Rate limits alone do not bypass enterprise bot protection (Akamai, Cloudflare). Each store is assigned a transport tier:

| Tier | Stores | Transport |
|------|--------|-----------|
| API | Amazon, REI (partial) | Affiliate API — no scraping |
| Clean HTTP | CampSaver, Garage Grown Gear | `reqwest` + cookie jar + browser headers |
| Proxy rotation | Backcountry, Moosejaw, Steep & Cheap | Bright Data / Zyte residential proxy pool |
| Headless browser | Cabela's / Bass Pro | `chromiumoxide` (Rust CDP) as fallback |

The `StoreCrawler` trait abstracts the transport. Each implementation selects its tier. Upgrading a store from HTTP to headless is a one-file change.

### Headless Browser Pool (Tier 4 — Cabela's)

Running one Chrome instance per scrape URL exhausts RAM on a 1GB machine within minutes (each Chrome instance: 100–300MB). The correct model is a fixed-size pool with tab reuse inside a single long-lived browser process:

```rust
pub struct BrowserPool {
    semaphore: Arc<Semaphore>,       // caps concurrent page operations (max 3)
    browser: Arc<Mutex<Browser>>,    // single reused chromiumoxide Browser instance
}

impl BrowserPool {
    pub async fn scrape<F, T>(&self, url: &str, f: F) -> Result<T>
    where
        F: AsyncFnOnce(Page) -> Result<T>
    {
        let _permit = self.semaphore.acquire().await?;   // blocks if 3 already active
        let page = {
            let browser = self.browser.lock().await;
            browser.new_page(url).await?                 // new tab, not new browser
        };
        let result = f(page.clone()).await;
        page.close().await.ok();                         // return tab slot
        result
    }
}
```

One browser instance, semaphore at 3 concurrent tabs, tabs closed after each use. Cabela's runs last in the daily pipeline, after all HTTP-tier stores complete — total memory at peak is ~200–400MB for 3 simultaneous Chrome tabs, well within budget.

```rust
// Browser-like headers for clean HTTP tier
fn browser_headers() -> HeaderMap {
    let mut h = HeaderMap::new();
    h.insert(USER_AGENT, "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) ...".parse().unwrap());
    h.insert(ACCEPT_LANGUAGE, "en-US,en;q=0.9".parse().unwrap());
    h.insert(ACCEPT_ENCODING, "gzip, deflate, br".parse().unwrap());
    h
}
```

Proxy integration is configured per-store via `SCRAPE_PROXY_{STORE_ID}` env var. If unset, the store uses direct HTTP.

```rust
// Per-store rate limits (requests/sec) — courtesy limits, not bot bypass
const STORE_RATE_LIMITS: &[(&str, u32)] = &[
    ("amazon", 1),          // PA API has strict quota
    ("rei", 3),
    ("backcountry", 2),
    ("cabelas", 2),
    ("moosejaw", 3),
    ("steepandcheap", 3),
    ("campsaver", 2),
    ("garagerowngear", 1),  // Small indie site — be respectful
];
```

- Respect `robots.txt` for all scraped stores
- User-Agent identifies GearNest with contact email
- Garage Grown Gear: lowest rate limit — small site, treat gently

---

## 8. Java RAG Orchestrator (Spring Boot)

### Project Structure

```
gear-nest-api/
├── src/main/java/io/gearnest/api/
│   ├── GearNestApiApplication.java
│   ├── config/
│   │   ├── AiConfig.java           # Spring AI HuggingFace config
│   │   ├── RedisConfig.java
│   │   └── SecurityConfig.java     # CORS, rate limiting
│   ├── product/
│   │   ├── ProductController.java
│   │   ├── ProductService.java
│   │   ├── ProductRepository.java  # JPA + pgvector queries
│   │   └── dto/
│   ├── search/
│   │   ├── SearchController.java
│   │   └── SearchService.java      # Hybrid: pgvector + FTS
│   ├── rag/
│   │   ├── ChatController.java     # SSE endpoint
│   │   ├── RagService.java         # Query → embed → retrieve → generate
│   │   └── PromptBuilder.java
│   ├── review/
│   │   ├── ReviewController.java
│   │   ├── ReviewService.java
│   │   └── SummaryService.java     # AI summary generation + cache
│   ├── pricing/
│   │   ├── PricingController.java
│   │   └── PricingService.java     # Best-value ranking
│   └── session/
│       └── SessionBudgetService.java   # Token limit tracking
```

### Spring AI Configuration

```java
@Configuration
public class AiConfig {
    
    @Bean
    public EmbeddingModel embeddingModel() {
        // HuggingFace BAAI/bge-small-en-v1.5
        return new HuggingFaceEmbeddingModel(
            HuggingFaceEmbeddingOptions.builder()
                .withModel("BAAI/bge-small-en-v1.5")
                .build()
        );
    }

    @Bean
    public ChatModel chatModel() {
        // Mistral-7B-Instruct for generation
        return new HuggingFaceChatModel(
            HuggingFaceChatOptions.builder()
                .withModel("mistralai/Mistral-7B-Instruct-v0.3")
                .withMaxTokens(512)
                .withTemperature(0.3f)   // low temp = factual responses
                .build()
        );
    }
}
```

### SSE Streaming (Chat)

```java
@GetMapping(value = "/api/chat", produces = MediaType.TEXT_EVENT_STREAM_VALUE)
public SseEmitter chat(
    @RequestParam String query,
    @RequestParam String productId,
    @CookieValue(required = false) String sessionId,
    HttpServletResponse response
) {
    SseEmitter emitter = new SseEmitter(60_000L);
    
    // 1. Check session budget
    if (!sessionBudgetService.hasRemainingBudget(sessionId)) {
        emitter.send(SseEmitter.event().name("limit_reached").data("{}"));
        emitter.complete();
        return emitter;
    }
    
    // 2. RAG + stream on virtual thread
    Thread.ofVirtual().start(() -> {
        try {
            ragService.streamAnswer(query, productId, sessionId, emitter);
        } catch (Exception e) {
            emitter.completeWithError(e);
        }
    });
    
    return emitter;
}
```

Java 21 virtual threads handle SSE without blocking the web thread pool.

### RAG Service Flow

```java
public void streamAnswer(String query, String productId, String sessionId, SseEmitter emitter) {
    // 1. Reserve session budget (reserve-then-commit — see §14)
    //    Deducts 1 question immediately; rolls back on HF timeout before first token
    boolean reserved = sessionBudgetService.reserve(sessionId);
    if (!reserved) {
        emitter.send(SseEmitter.event().name("limit_reached").data("{}"));
        emitter.complete();
        return;
    }

    // 2. Embed the user query
    float[] queryEmbedding = embeddingModel.embed(query);

    // 3. Stratified retrieval — balanced sentiment, diversity-aware (see §11)
    //    semantic top-5 (MMR) + top-3 negative review chunks + top-2 spec chunks
    List<Chunk> semanticChunks = vectorRepository.findSimilarReviewsMmr(productId, queryEmbedding, 5, 0.7f);
    List<Chunk> negativeChunks = vectorRepository.findNegativeReviews(productId, queryEmbedding, 3);
    List<Chunk> specChunks    = vectorRepository.findSimilarSpecs(productId, queryEmbedding, 2);

    // 4. Build grounded prompt
    String prompt = promptBuilder.build(query, semanticChunks, negativeChunks, specChunks);

    // 5. Stream LLM response — commit budget on first token received
    boolean[] committed = {false};
    chatModel.stream(prompt)
        .doOnNext(token -> {
            if (!committed[0]) {
                sessionBudgetService.commit(sessionId);  // first token = question consumed
                committed[0] = true;
            }
            emitter.send(SseEmitter.event().data(token));
        })
        .doOnError(err -> {
            if (!committed[0]) {
                sessionBudgetService.rollback(sessionId);  // HF never responded — restore budget
                emitter.send(SseEmitter.event().name("error")
                    .data("{\"budgetRestored\": true}"));
            }
            emitter.completeWithError(err);
        })
        .blockLast();

    emitter.send(SseEmitter.event().name("done").data(
        Map.of("remaining", sessionBudgetService.getRemaining(sessionId))
    ));
    emitter.complete();
}
```

### Hybrid Search (pgvector + Full-Text)

For the product catalog search bar (not the AI chat — that's pure semantic):

```java
// Combine vector similarity score + full-text rank
@Query(value = """
    SELECT p.*, 
           (0.6 * (1 - (sc.embedding <=> :embedding::vector))) 
           + (0.4 * ts_rank(to_tsvector('english', p.name || ' ' || p.brand), 
                            plainto_tsquery('english', :query))) AS score
    FROM products p
    JOIN spec_chunks sc ON sc.product_id = p.id
    WHERE sc.embedding <=> :embedding::vector < 0.4
       OR to_tsvector('english', p.name || ' ' || p.brand) 
          @@ plainto_tsquery('english', :query)
    ORDER BY score DESC
    LIMIT :limit
    """, nativeQuery = true)
List<ProductSearchResult> hybridSearch(
    @Param("query") String query,
    @Param("embedding") float[] embedding,
    @Param("limit") int limit
);
```

This ensures "Columbia rain jacket" (exact match) and "waterproof layer for Pacific Northwest" (semantic) both return good results.

---

## 9. Next.js Frontend

### Route Structure

```
app/
├── page.tsx                    # Home: hero + featured categories + top picks
├── products/
│   ├── page.tsx                # Catalog: search + filter + grid
│   └── [slug]/
│       └── page.tsx            # Product detail: full page
├── category/
│   └── [slug]/
│       └── page.tsx            # Category browse page
├── about/
│   └── page.tsx
└── layout.tsx                  # Nav + footer + theme provider
```

### Product Detail Page Layout

```
┌─────────────────────────────────────────────────────────────────┐
│ [Image Gallery]     │  Product Name, Brand                      │
│                     │  Aggregate Rating: ★★★★☆ (4.2 / 1,847 rev)│
│                     │  ─────────────────────────────────────    │
│                     │  PRICE COMPARISON TABLE (sorted best val) │
│                     │  ┌──────────────┬────────┬───────┬──────┐ │
│                     │  │ Store        │ Price  │Rating │ Link │ │
│                     │  ├──────────────┼────────┼───────┼──────┤ │
│                     │  │ REI ★        │ $289   │ 4.4★  │  →  │ │
│                     │  │ Backcountry  │ $299   │ 4.3★  │  →  │ │
│                     │  │ Amazon       │ $312   │ 4.1★  │  →  │ │
│                     │  └──────────────┴────────┴───────┴──────┘ │
│                     │  Prices as of 2 hrs ago · Next update: 22h│
├─────────────────────┴──────────────────────────────────────────┤
│  SPECS TAB  │  REVIEWS TAB  │  ASK AI TAB                       │
├────────────────────────────────────────────────────────────────┤
│  [Active tab content]                                           │
│                                                                 │
│  REVIEWS TAB:                                                   │
│  Community Verdict (AI-generated):                              │
│  "Reviewers consistently praise the warmth-to-weight ratio and  │
│   packability. Common complaints focus on the zipper durability │
│   after 6+ months and that the hood is not helmet-compatible."  │
│                                                                 │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ ★★★★★ (634 reviews)                                      │  │
│  │ [Review 1 from REI, verified] [Review 2 from Amazon]    │  │
│  ├──────────────────────────────────────────────────────────┤  │
│  │ ★★★★☆ (812 reviews)                                      │  │
│  │ [Review 1 from Backcountry] [Review 2 from REI]         │  │
│  ├──────────────────────────────────────────────────────────┤  │
│  │ ★★★☆☆ (243 reviews) ...                                  │  │
│  └──────────────────────────────────────────────────────────┘  │
│                                                                 │
│  ASK AI TAB:                                                    │
│  ┌──────────────────────────────────────────────────────────┐  │
│  │ [Chat messages with streaming...]                        │  │
│  │ You have 3 questions remaining this session.             │  │
│  │ ┌────────────────────────────┐ [Ask]                    │  │
│  │ │ Ask about this product...  │                          │  │
│  │ └────────────────────────────┘                          │  │
│  └──────────────────────────────────────────────────────────┘  │
└────────────────────────────────────────────────────────────────┘
```

### Key Components

**`PriceComparisonTable`**
- Sorted by best-value score (see §12)
- Best value store gets a "Best Value" badge
- Shows: store logo, price, store rating, in-stock indicator, affiliate/direct link button
- Footer: "Prices fetched [relative time] · Next update [relative time]"
- Skeleton loader while fetching

**`CommunityVerdict`**
- AI-generated, cached per product
- Displayed as a card above reviews
- Shows: summary paragraph + pros (green bullets) + cons (amber bullets)
- Badge: "Generated from [N] reviews across [M] stores"
- If not yet generated (new product): shows "Verdict generating..." with async trigger

**`ChatInterface`**
- Tab on product detail page (context is this specific product)
- SSE consumer with typewriter effect on streaming tokens
- Session budget indicator: "3 questions remaining" with tooltip explaining limit
- Suggested questions auto-populated from product category (e.g., "Is this waterproof?", "Compare to similar stoves")
- After limit reached: "Session limit reached. Come back tomorrow or [open in new tab] to start fresh."

**`ReviewTierSection`**
- Expandable section per star tier (5★, 4★, 3★, 2★, 1★)
- Shows 2 reviews per tier by default (most helpful within tier)
- Review card: rating, title, body truncated at 200 chars, store badge, date, "Verified Purchase" badge where available
- "See all [N] reviews from REI" link per store

---

## 10. API Contracts

### Product Search
```
GET /api/v1/products/search
  ?q=camping+stove+high+altitude
  &category=camp-kitchen
  &brand=MSR,Jetboil
  &min_price=50
  &max_price=300
  &sort=best_value|price_asc|rating_desc
  &page=1&size=24

Response: {
  products: [ProductCard],
  total: 4821,
  page: 1,
  facets: { brands: [...], categories: [...], price_ranges: [...] }
}
```

### Product Detail
```
GET /api/v1/products/{slug}

Response: {
  id, slug, name, brand, category, description, specs: {...},
  images: [...],
  aggregateRating: { average: 4.2, count: 1847 },
  listings: [StoreListing],  // sorted by best_value_score
  pricesLastUpdated: "2026-05-27T06:00:00Z",
  pricesNextUpdate: "2026-05-28T06:00:00Z"
}
```

### Price Comparison
```
GET /api/v1/products/{id}/prices

Response: {
  listings: [{
    store: { id, displayName, logoUrl },
    price: 289.99,
    affiliateUrl: "...",
    storeRating: 4.4,
    reviewCount: 312,
    inStock: true,
    bestValueScore: 0.87,
    isBestValue: true
  }],
  lastUpdated: "...",
  nextUpdate: "..."
}
```

### Reviews
```
GET /api/v1/products/{id}/reviews
  ?tier=5&page=1&size=2

Response: {
  tiers: {
    "5": { count: 634, sample: [Review, Review] },
    "4": { count: 812, sample: [Review, Review] },
    ...
  },
  total: 1847,
  storeBreakdown: [{ store: "rei", count: 312, avgRating: 4.4 }]
}
```

### AI Summary
```
GET /api/v1/products/{id}/summary

Response: {
  summary: "Reviewers consistently praise...",
  pros: ["Excellent warmth-to-weight ratio", "Packs to fist size"],
  cons: ["Zipper durability after heavy use", "Hood not helmet-compatible"],
  reviewCount: 1847,
  generatedAt: "2026-05-26T14:30:00Z"
}
```

### Chat (SSE)
```
GET /api/v1/chat
  ?query=Is+this+good+for+below+freezing+conditions
  &productId=abc-123
  Cookie: gn_session=<session-id>

Content-Type: text/event-stream

event: token
data: "Based"

event: token
data: " on"

... (streaming)

event: done
data: {"remaining": 3, "sessionId": "..."}

event: limit_reached   (if budget exhausted before query)
data: {}
```

---

## 11. RAG System Design

### Why RAG for this product

Traditional keyword search can answer "MSR Reactor stove price" but fails at "which stove performs best in below-freezing conditions at altitude?" RAG retrieves relevant spec chunks (boiling time at altitude, BTU output) and review chunks (user reports from Denali, Colorado 14ers) and generates a synthesized, grounded answer.

### Retrieval Strategy

```
User query: "Which camping stove is best for high altitude and freezing temps?"

Step 1 — Embed query
  BAAI/bge-small-en-v1.5 → query_vector (384d)

Step 2 — Exact KNN retrieval (product-scoped, no HNSW needed)
  Per-product chunk count: ~60-80 review chunks + ~10 spec chunks = ~75 rows
  PostgreSQL filters by product_id via btree, then does exact cosine over ~75 rows.
  This is faster than HNSW on a 99.98%-filtered graph. No index degradation.

  SELECT * FROM review_chunks
  WHERE product_id = ?
  ORDER BY embedding <=> query_vector   -- exact cosine, ~75 rows
  LIMIT 20;                             -- over-fetch for MMR + stratification

Step 3 — Stratified selection (balanced sentiment + diversity)

  Slot A — Semantic top-5 via MMR (Maximal Marginal Relevance, λ=0.7):
    Iteratively select chunks that maximize:
      λ × sim(chunk, query) - (1-λ) × max(sim(chunk, already_selected))
    Prevents 5 slots from being near-paraphrases of each other.

  Slot B — Top-3 negative review chunks (rating ≤ 2):
    SELECT * FROM review_chunks
    WHERE product_id = ? AND rating <= 2
    ORDER BY embedding <=> query_vector
    LIMIT 3;
    Guarantees failure modes are represented regardless of query direction.
    ("Is this good for rain?" won't miss the snow failure.)

  Slot C — Top-2 spec chunks (product specs, not reviews):
    Grounds the response in manufacturer-stated specs.

  Final context: 5 + 3 + 2 = 10 chunks

Step 4 — Prompt construction
  System: "You are GearNest AI, an expert outdoor gear advisor.
            Answer only from the provided context.
            If the context does not contain enough information, say so.
            Be concise — 3-5 sentences max."

  Context: [10 chunks labeled as SPEC, POSITIVE_REVIEW, or CRITICAL_REVIEW]

  User: "Which camping stove is best for high altitude and freezing temps?"

Step 5 — Stream response via Mistral-7B-Instruct
```

### Guardrails

- **Hallucination prevention:** Explicit system prompt instruction to answer only from context
- **Context limit:** Hard cap of 10 retrieved chunks → ~2,000 tokens context window
- **Off-topic guard:** If query is unrelated to the product (e.g., "what's the weather in Denver?"), Java service detects via keyword heuristic and returns a canned response without calling the LLM
- **Product scope:** Chat is always product-scoped. Global "which stove is best overall?" queries are handled by the catalog search, not the chat.

### AI Summary Generation (Background)

Product summaries are generated once and cached:

```
Trigger: New product indexed OR review count increased by >50 since last generation

Input: Top 40 review chunks from pgvector (broad cosine threshold 0.7)
Prompt: "Summarize what customers love and dislike about this product 
         based on the reviews below. Output JSON: 
         {summary: string, pros: string[], cons: string[]}"

Cache: Redis + ai_summaries table (invalidated when review_count delta > 50)
```

---

## 12. Price Comparison Logic

### Best Value Score

```
best_value_score = (0.4 × normalized_price_score) + (0.6 × store_rating_score)

normalized_price_score = (max_price - store_price) / (max_price - min_price)
  → cheapest store gets 1.0, most expensive gets 0.0

store_rating_score = store_rating / 5.0
  → 5.0★ store gets 1.0, unrated gets 0.5

Tiebreaker: if scores within 0.02 of each other, rank by price ascending
```

**Example:**

| Store | Price | Store Rating | Price Score | Rating Score | Best Value Score |
|-------|-------|-------------|-------------|--------------|-----------------|
| REI | $289 | 4.4★ | 0.88 | 0.88 | **0.88** ← Best |
| Backcountry | $299 | 4.3★ | 0.59 | 0.86 | 0.75 |
| Amazon | $312 | 4.1★ | 0.0 | 0.82 | 0.49 |

### Display Logic

- Best value store gets a green "Best Value" badge
- In-stock indicator shown per store (out-of-stock listed last)
- Price shown with 2 decimal places + currency
- "Prices as of X · Next update in Y" displayed below table
- Price trend (up/down arrow + delta) if price changed from last sync

---

## 13. Review Aggregation Strategy

### Collection

For each product × store combination:
- Collect up to 500 reviews per store (pagination-aware scraping)
- Amazon PA API provides up to 10 reviews per product — supplement with scraping product page
- Store `source_review_id` for exact deduplication per store
- Hash `reviewer_id` (where available) for cross-store near-deduplication

### Cross-Store Deduplication

Naive Jaccard over all review pairs per product is O(n²). With ~500 reviews per product × 50,000 products, that is not viable in a batch job. Two-stage approach runs in O(n) average:

**Stage 1 — Exact dedup (O(1) per review, handles ~90% of cases):**
- `UNIQUE(store_id, source_review_id)` already prevents same-store re-imports.
- `reviewer_id_hash` match across stores: same reviewer leaving a review on Amazon AND REI → keep the one with `verified_purchase = true`, discard the other.

**Stage 2 — Near-dedup via MinHash LSH (for anonymous reviews without reviewer_id):**

Short reviews bypass Stage 2 entirely. Under 150 characters ("Great product, fast shipping!"), 3-gram word shingles produce nearly identical token sets across distinct authentic reviews — the MinHash similarity score becomes meaningless, causing false-positive deduplication of legitimate content. The UNIQUE constraint already handles identical short reviews from the same store; cross-store dedup of short texts adds no value.

```rust
const MIN_DEDUP_CHARS: usize = 150;

fn should_run_stage2(review: &RawReview) -> bool {
    // Only near-dedup multi-paragraph reviews — short reviews are safe to skip
    review.reviewer_id_hash.is_none() && review.body.len() >= MIN_DEDUP_CHARS
}
```

For reviews that pass the threshold:

```rust
use minhash_lsh::{MinHash, Lsh};

// Word-level 3-grams on reviews >= 150 chars
// LSH buckets: reviews with similar signatures are candidates
// Only compare reviews within the same bucket → O(n) average

fn build_minhash(text: &str) -> MinHash {
    let words: Vec<&str> = text.split_whitespace().collect();
    let shingles: HashSet<String> = words.windows(3)
        .map(|w| w.join(" "))
        .collect();
    MinHash::from_shingles(&shingles, 128)
}

// Threshold: estimated Jaccard ≥ 0.85 within LSH bucket → duplicate
// Keep: verified_purchase = true → helpful_votes DESC → most recent
```

Stage 2 runs only for the ~15% of reviews lacking a `reviewer_id_hash` AND with body length ≥ 150 chars. This targets the actual problem: multi-paragraph syndicated corporate reviews copied across retailer sites.

### Per-Tier Sample Selection (for UI)

For the "2 reviews per tier" display, select by:
1. Filter by star tier (e.g., 5★ reviews)
2. Prefer `verified_purchase = true`
3. Sort by `helpful_votes DESC`
4. Take top 2

This surfaces the most credible, community-validated reviews per tier.

### Aggregate Rating Calculation

```
aggregate_rating = weighted average of store ratings

weight per store = log(1 + store_review_count)  // diminishing returns on volume
                                                  // avoids Amazon dominating with 10k reviews
```

---

## 14. Session Management & LLM Limits

### Why Sessions Without Auth

Requiring login to ask questions kills casual discovery. Sessions via cookies provide lightweight budget tracking without a registration barrier.

### Session Budget

```
Per-session budget:
  - 5 AI questions per session
  - Session window: 2 hours of inactivity resets the session (new cookie)
  - No cross-session persistence (intentional — fresh sessions daily)

Redis schema:
  key:   "session:{session_id}:questions_remaining"   value: int (5→0)  TTL: 2hr
  key:   "session:{session_id}:inflight"              value: "1"        TTL: 90s
```

### Reserve-then-Commit Protocol

Tokens are NOT deducted before the API call (user loses budget on timeout) nor strictly after success (undefined for streaming). Instead:

```
At question submit:
  1. WATCH session:{id}:questions_remaining
  2. If 0: reject with limit_reached event, return.
  3. MULTI
       DECR session:{id}:questions_remaining   ← shown in UI immediately ("4 remaining")
       SET  session:{id}:inflight 1 EX 90      ← 90-sec reservation window
     EXEC

On first token received from HuggingFace:
  4. DEL session:{id}:inflight                 ← commit: question is consumed

On HuggingFace timeout (no first token within 30s):
  5. INCR session:{id}:questions_remaining     ← rollback: restore budget
  6. DEL  session:{id}:inflight
  7. Send SSE event: {type:"error", budgetRestored:true}
     UI shows: "Request timed out. Your question was not counted."

Safety valve:
  If server crashes between steps 3 and 4, the 90s TTL on "inflight" auto-expires.
  Next question check finds inflight = nil and questions_remaining already decremented.
  Add a startup reconciliation: on boot, INCR any session with inflight=1 and no
  in-progress stream. Worst case: user gets one free question back on server restart.
```

Mid-stream failures (first token received, then stream dies): budget is already committed. User received a partial answer — treated as consumed, which is fair.

### UI Representation

```
"3 questions remaining this session"
[Progress indicator: ●●●○○]

On limit:
"You've reached your session limit. 
 Start a new session or explore more products."
[Start new session]  ← clears cookie, issues new session
```

### Why Not Unlimited?

HuggingFace Pro inference has rate limits. 5 questions × N concurrent users needs to stay within the free tier. This is explicitly explained in the portfolio writeup as a conscious cost-management architectural decision.

---

## 15. Deployment Architecture

### 15.1 Local Development

```yaml
# docker-compose.yml
services:
  postgres:
    image: pgvector/pgvector:pg16
    ports: ["5432:5432"]
    volumes: ["pgdata:/var/lib/postgresql/data"]
    environment:
      POSTGRES_DB: gearnest
      POSTGRES_USER: gearnest
      POSTGRES_PASSWORD: gearnest_dev

  redis:
    image: redis:7-alpine
    ports: ["6379:6379"]

  pipeline:              # Rust — run manually for initial seed
    build: ./gear-nest-pipeline
    depends_on: [postgres]
    environment:
      DATABASE_URL: postgresql://gearnest:gearnest_dev@postgres:5432/gearnest

  api:                   # Java Spring Boot
    build: ./gear-nest-api
    ports: ["8080:8080"]
    depends_on: [postgres, redis]

  frontend:              # Next.js dev server
    build: ./gear-nest-web
    ports: ["3000:3000"]
    depends_on: [api]
```

---

### 15.2 Production — GCP Hybrid Architecture

Fly.io replaced with GCP for enterprise-grade deployment signals (Cloud Run, Cloud SQL, IAM, Workload Identity, VPC). Redis stays on Upstash to avoid GCP Memorystore's ~$35/month minimum cost floor.

**ADR-012: GCP over Fly.io — see §18.**

```
GitHub (main branch)
        │
        ▼
GitHub Actions (CI/CD)
  ├── build & push → Artifact Registry (us-central1)
  ├── deploy → Cloud Run (Java API)
  └── trigger → Cloud Run Job (Rust pipeline, daily)

Cloud Run (Java API)           Vercel (Next.js)
  │  VPC connector               │
  ▼                              │ (public HTTPS)
Cloud SQL                  ──────┘
  PostgreSQL 16 + pgvector
  (private VPC — no public IP)

Upstash Redis (external)   ◄── Cloud Run + pipeline
                                (TLS, token auth)
```

**Service map:**

| Component | Platform | Spec | Est. Cost/mo |
|-----------|----------|------|--------------|
| Next.js frontend | Vercel | Hobby plan | Free |
| Java API | Cloud Run | 1 vCPU, 512 MB, min-instances: 1 | ~$8 |
| PostgreSQL 16 + pgvector | Cloud SQL | `db-f1-micro`, 10 GB SSD | ~$10 |
| Redis (session + price cache) | Upstash | Free tier (10K req/day) | Free |
| Rust pipeline | Cloud Run Jobs | 1 vCPU, 1 GB, daily schedule | ~$1 |
| Container images | Artifact Registry | ~2 GB storage | ~$0.20 |
| Cloud Scheduler | GCP | 3 jobs (free tier) | Free |
| Cloud Monitoring + Logging | GCP | Free tier (50 GB logs/mo) | Free |
| **Total** | | | **~$19–20/mo** |

**Hard budget ceiling: $30/month — see §15.5 for enforcement.**

---

### 15.3 GCP Project Setup

**One-time setup (run in order):**

```bash
# 1. Create project
gcloud projects create gearnest-prod --name="GearNest"
gcloud config set project gearnest-prod

# 2. Link billing account
gcloud billing projects link gearnest-prod \
  --billing-account=YOUR_BILLING_ACCOUNT_ID

# 3. Enable required APIs
gcloud services enable \
  run.googleapis.com \
  sqladmin.googleapis.com \
  artifactregistry.googleapis.com \
  cloudscheduler.googleapis.com \
  secretmanager.googleapis.com \
  vpcaccess.googleapis.com \
  iam.googleapis.com \
  billingbudgets.googleapis.com

# 4. Create Artifact Registry
gcloud artifacts repositories create gearnest \
  --repository-format=docker \
  --location=us-central1
```

---

### 15.4 IAM & Workload Identity

No passwords in environment variables for Cloud Run → Cloud SQL. Use Workload Identity + Cloud SQL Auth Proxy sidecar instead.

**Service accounts:**

```bash
# API service account
gcloud iam service-accounts create gearnest-api \
  --display-name="GearNest API"

# Pipeline service account
gcloud iam service-accounts create gearnest-pipeline \
  --display-name="GearNest Pipeline"

# Grant Cloud SQL Client to both
for SA in gearnest-api gearnest-pipeline; do
  gcloud projects add-iam-policy-binding gearnest-prod \
    --member="serviceAccount:${SA}@gearnest-prod.iam.gserviceaccount.com" \
    --role="roles/cloudsql.client"
done

# Grant Artifact Registry reader to Cloud Run (pull images)
gcloud projects add-iam-policy-binding gearnest-prod \
  --member="serviceAccount:gearnest-api@gearnest-prod.iam.gserviceaccount.com" \
  --role="roles/artifactregistry.reader"
```

**Cloud SQL Auth Proxy pattern (in Cloud Run service YAML):**

```yaml
# cloud-run-api.yaml
apiVersion: serving.knative.dev/v1
kind: Service
metadata:
  name: gearnest-api
spec:
  template:
    metadata:
      annotations:
        run.googleapis.com/cloudsql-instances: gearnest-prod:us-central1:gearnest-db
        autoscaling.knative.dev/minScale: "1"
        autoscaling.knative.dev/maxScale: "3"
    spec:
      serviceAccountName: gearnest-api@gearnest-prod.iam.gserviceaccount.com
      containers:
        - image: us-central1-docker.pkg.dev/gearnest-prod/gearnest/api:latest
          resources:
            limits:
              cpu: "1"
              memory: 512Mi
          env:
            - name: SPRING_DATASOURCE_URL
              value: jdbc:postgresql:///gearnest?cloudSqlInstance=gearnest-prod:us-central1:gearnest-db&socketFactory=com.google.cloud.sql.postgres.SocketFactory
            - name: REDIS_URL
              valueFrom:
                secretKeyRef:
                  name: upstash-redis-url
                  key: latest
            - name: HUGGINGFACE_API_KEY
              valueFrom:
                secretKeyRef:
                  name: huggingface-api-key
                  key: latest
```

Secrets (Redis URL, HuggingFace key, Amazon PA API keys) live in **Secret Manager** — never in Cloud Run env vars as plaintext.

```bash
# Store secrets
echo -n "redis://..." | gcloud secrets create upstash-redis-url --data-file=-
echo -n "hf_..."      | gcloud secrets create huggingface-api-key --data-file=-
echo -n "..."          | gcloud secrets create amazon-pa-api-key --data-file=-
echo -n "..."          | gcloud secrets create amazon-pa-secret --data-file=-

# Grant Cloud Run SA access to secrets
gcloud secrets add-iam-policy-binding upstash-redis-url \
  --member="serviceAccount:gearnest-api@gearnest-prod.iam.gserviceaccount.com" \
  --role="roles/secretmanager.secretAccessor"
```

---

### 15.5 Cost Management & Budget Alerts (Hard $30/month Ceiling)

#### Step 1 — Create a GCP Budget

```bash
gcloud billing budgets create \
  --billing-account=YOUR_BILLING_ACCOUNT_ID \
  --display-name="GearNest $30 Hard Cap" \
  --budget-amount=30USD \
  --threshold-rule=percent=50,basis=CURRENT_SPEND \
  --threshold-rule=percent=90,basis=CURRENT_SPEND \
  --threshold-rule=percent=100,basis=CURRENT_SPEND \
  --notifications-rule-pubsub-topic=projects/gearnest-prod/topics/billing-alerts \
  --notifications-rule-monitoring-notification-channels=EMAIL
```

Or in Cloud Console: **Billing → Budgets & Alerts → Create Budget**

| Threshold | Amount | Action |
|-----------|--------|--------|
| 50% | ~$15 | Email alert only |
| 90% | ~$27 | Email alert — review what's running |
| 100% | $30 | Email + **auto-disable billing** (see below) |

#### Step 2 — Auto-Disable Billing at $30 (Hard Stop)

This Cloud Function fires when the budget Pub/Sub alert fires at 100% and disables billing on the project entirely — which stops all running services.

```bash
# Create Pub/Sub topic for budget alerts
gcloud pubsub topics create billing-alerts

# Deploy the auto-stop function
gcloud functions deploy billing-auto-stop \
  --runtime=python312 \
  --trigger-topic=billing-alerts \
  --entry-point=stop_billing \
  --region=us-central1 \
  --service-account=gearnest-pipeline@gearnest-prod.iam.gserviceaccount.com
```

`functions/billing_auto_stop/main.py`:

```python
import base64, json
from googleapiclient import discovery

PROJECT_ID = "gearnest-prod"

def stop_billing(event, context):
    data = json.loads(base64.b64decode(event["data"]).decode())
    cost_amount   = float(data["costAmount"])
    budget_amount = float(data["budgetAmount"])

    if cost_amount < budget_amount:
        return  # alert fired below 100% — email only, no action

    billing = discovery.build("cloudbilling", "v1", cache_discovery=False)
    billing.projects().updateBillingInfo(
        name=f"projects/{PROJECT_ID}",
        body={"billingAccountName": ""},   # empty string = disable billing
    ).execute()
    print(f"Billing disabled. Spend ${cost_amount:.2f} reached ${budget_amount:.2f} cap.")
```

Grant the pipeline SA permission to disable billing:

```bash
gcloud projects add-iam-policy-binding gearnest-prod \
  --member="serviceAccount:gearnest-pipeline@gearnest-prod.iam.gserviceaccount.com" \
  --role="roles/billing.projectManager"
```

> **Note:** Once billing is disabled, all Cloud Run services and Cloud SQL stop. To re-enable: Cloud Console → Billing → Link a billing account. Cloud SQL data is preserved for 30 days.

#### Step 3 — Cost Visibility (daily check)

```bash
# Quick spend-to-date check
gcloud billing accounts get-iam-policy YOUR_BILLING_ACCOUNT_ID
gcloud alpha billing accounts describe YOUR_BILLING_ACCOUNT_ID

# Or set a Cloud Monitoring dashboard alert for daily spend > $1.50
# (flags a runaway service before it compounds)
```

---

### 15.6 CI/CD — GitHub Actions

Three workflows, one per deployable service:

**`.github/workflows/deploy-api.yml`**
```yaml
name: Deploy API
on:
  push:
    branches: [main]
    paths: ["gear-nest-api/**"]

jobs:
  deploy:
    runs-on: ubuntu-latest
    permissions:
      contents: read
      id-token: write          # Workload Identity Federation

    steps:
      - uses: actions/checkout@v4

      - uses: google-github-actions/auth@v2
        with:
          workload_identity_provider: ${{ secrets.WIF_PROVIDER }}
          service_account: gearnest-api@gearnest-prod.iam.gserviceaccount.com

      - uses: google-github-actions/setup-gcloud@v2

      - name: Build & push
        run: |
          docker build -t us-central1-docker.pkg.dev/gearnest-prod/gearnest/api:${{ github.sha }} ./gear-nest-api
          docker push us-central1-docker.pkg.dev/gearnest-prod/gearnest/api:${{ github.sha }}

      - name: Deploy to Cloud Run
        run: |
          gcloud run deploy gearnest-api \
            --image=us-central1-docker.pkg.dev/gearnest-prod/gearnest/api:${{ github.sha }} \
            --region=us-central1 \
            --no-traffic \
            --tag=candidate
          # Zero-downtime: shift traffic only after health check passes
          gcloud run services update-traffic gearnest-api \
            --to-tags=candidate=100 --region=us-central1
```

**`.github/workflows/deploy-web.yml`** — Vercel handles this automatically via git integration. No workflow needed.

**`.github/workflows/test.yml`**
```yaml
name: Test
on: [pull_request]

jobs:
  pipeline:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cd gear-nest-pipeline && cargo test
      - run: cd gear-nest-pipeline && cargo clippy -- -D warnings

  api:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-java@v4
        with: { java-version: "21", distribution: "temurin" }
      - run: cd gear-nest-api && ./mvnw test

  web:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - uses: actions/setup-node@v4
        with: { node-version: "24" }
      - run: cd gear-nest-web && npm ci && npm run typecheck && npm run lint && npm run build
```

---

### 15.7 Environment Variables

Secrets live in **GCP Secret Manager** — never in plaintext env vars or `.env` files committed to git.

```
# GCP Secret Manager keys (reference these in Cloud Run YAML)
upstash-redis-url          → REDIS_URL for API + pipeline
huggingface-api-key        → HF inference calls
amazon-pa-api-key          → Amazon PA API
amazon-pa-secret           → Amazon PA API
amazon-associate-tag       → gearnest-20

# Non-secret runtime config (Cloud Run env vars, safe to commit in YAML)
SESSION_QUESTION_LIMIT=5
HUGGINGFACE_EMBEDDING_MODEL=BAAI/bge-small-en-v1.5
HUGGINGFACE_LLM_MODEL=mistralai/Mistral-7B-Instruct-v0.2
GCP_PROJECT=gearnest-prod
GCP_REGION=us-central1
```

Local development uses `.env.local` (gitignored). See `.env.example` for required keys.

---

## 16. Build Phases

### Monorepo Layout

Single GitHub repo, three service directories, shared docs at root — mirrors MarketMind's `pipeline/` + Next.js root pattern:

```
gear-nest/
├── CHANGELOG.md            # Every feature logged — same discipline as MarketMind ADR 0001
├── README.md
├── CLAUDE.md               # Coding conventions per service (see below)
├── AGENTS.md               # Next.js breaking-change warning (same as MarketMind)
├── docker-compose.yml      # Local dev: postgres + redis + api + web
│
├── docs/
│   ├── ARCHITECTURE.md
│   ├── SETUP.md            # One-time setup steps
│   ├── RUNBOOK.md          # Recurring ops + incident playbooks
│   ├── DEPLOYMENT.md
│   └── adr/
│       ├── README.md
│       ├── 0001-documentation-as-rule.md
│       ├── 0002-monorepo-over-polyrepo.md
│       ├── 0003-pgvector-over-dedicated-vector-db.md
│       ... (matches §18 ADRs, numbered sequentially)
│
├── gear-nest-web/          # Next.js 16 frontend
│   ├── CLAUDE.md           # Web-specific: Tailwind v4, Server vs Client components
│   ├── app/
│   ├── components/
│   └── package.json
│
├── gear-nest-api/          # Java 21 + Spring Boot 3
│   ├── CLAUDE.md           # Java-specific: Spring AI patterns, virtual threads
│   ├── src/
│   └── pom.xml
│
└── gear-nest-pipeline/     # Rust ingestion pipeline
    ├── CLAUDE.md           # Rust-specific: async patterns, crate conventions
    ├── src/
    └── Cargo.toml
```

### CHANGELOG Format

Matches MarketMind exactly:

```
Format: YYYY-MM-DD · feature-name — one-line summary

## 2026-XX-XX — Phase 1 (in progress)

- **postgres-schema** — Initial schema with products, store_listings, reviews,
  review_chunks, spec_chunks, ai_summaries. pgvector extension enabled.
  btree indexes on product_id for chunk tables (no HNSW — see ADR-001).
  Migration: 001_initial_schema.sql.
```

### CLAUDE.md Conventions (root + per service)

Root CLAUDE.md links to service CLAUDEs. Each service CLAUDE.md includes:
- TypeScript strict / Rust 2021 edition / Java 21 with records and sealed types
- Import conventions (`@/` alias in web, package structure in Java)
- How to run the service locally
- Pre-push checklist (typecheck → lint → build/test)
- No comments unless WHY is non-obvious (matches MarketMind rule)

---

### Phase 1 — Foundation (Weeks 1–2)

**Goal:** Project scaffolded end-to-end, product catalog working with one store.

- [ ] Monorepo scaffold: root CLAUDE.md + AGENTS.md + CHANGELOG.md + README + docs/ structure + ADR 0001 (documentation rule)
- [ ] Per-service CLAUDE.md for gear-nest-web, gear-nest-api, gear-nest-pipeline
- [ ] Docker Compose local dev environment (postgres:pgvector16 + redis + api + web)
- [ ] PostgreSQL schema + pgvector extension (migration 001_initial_schema.sql)
- [ ] Rust: Amazon PA API crawler (products + basic metadata)
- [ ] Rust: DB loader with `ON CONFLICT DO UPDATE`
- [ ] Java Spring Boot: `/api/v1/products/search` + `/api/v1/products/{slug}`
- [ ] Next.js 16: Product catalog page + Product detail page (no AI, no prices yet)
- [ ] Portfolio entry stub: add GearNest to `lib/projects/data.ts` in personal-website with `status: 'Building'`
- [ ] Update CHANGELOG.md and README after each task above

**Milestone:** Browse 5,000 outdoor products from Amazon. Search by keyword. Docs discipline enforced from commit 1.

---

### Phase 2 — Multi-Store + Prices (Weeks 3–4)

**Goal:** Price comparison table live across all 8 stores.

- [ ] Rust: Scrapers for remaining 7 stores
- [ ] Rust: `price_sync` job with cron scheduler — writes to Redis hash + price_history
- [ ] Rust: Entity resolution (3-tier: GTIN/ASIN → brand+model extraction → embedding similarity)
- [ ] Rust: Brand alias table + model number regex patterns for Tier 2 matching
- [ ] Java: `/api/v1/products/{id}/prices` — Postgres static + Redis price enrichment
- [ ] Java: `CANDIDATE` confidence filter (exclude from user-facing API)
- [ ] Next.js: `PriceComparisonTable` component with "last updated" display

**Milestone:** Any product page shows prices from multiple stores, sorted by best value.

---

### Phase 3 — Reviews (Week 5)

**Goal:** Aggregated reviews with per-tier samples.

- [ ] Rust: Review scrapers for all stores (paginated)
- [ ] Rust: Cross-store review deduplication
- [ ] Java: `/api/v1/products/{id}/reviews` with tier breakdown
- [ ] Next.js: `ReviewTierSection` component

**Milestone:** Product page shows 1,847 total reviews, 2 authentic reviews per star tier.

---

### Phase 4 — RAG + AI (Weeks 6–7)

**Goal:** Chat interface and community verdict working.

- [ ] Rust: Text chunker (review fixed-size + spec sentence-boundary strategies)
- [ ] Rust: Embedding pipeline (HuggingFace batched calls → pgvector bulk insert)
- [ ] Rust: MinHash LSH for cross-store review deduplication (Stage 2)
- [ ] Java Spring AI: RAG service with stratified retrieval + MMR
- [ ] Java: SSE streaming endpoint `/api/v1/chat`
- [ ] Java: Session budget service (Redis-backed)
- [ ] Java: Background AI summary generation
- [ ] Next.js: `ChatInterface` with SSE consumer + typewriter effect
- [ ] Next.js: `CommunityVerdict` component

**Milestone:** Ask "Is this tent good for 4-season use?" and get a streaming answer grounded in reviews and specs.

---

### Phase 5 — Polish + Portfolio (Week 8)

**Goal:** Production-ready, portfolio-presentable.

- [ ] Hybrid search (pgvector + PostgreSQL FTS)
- [ ] Category pages with filters
- [ ] Product image gallery
- [ ] Mobile-responsive polish
- [ ] SEO: product page metadata + JSON-LD Product schema
- [ ] Production deploy: Fly.io + Vercel
- [ ] Portfolio website: Add GearNest to `/projects/[slug]` case study
- [ ] Record 3-min demo video for portfolio

---

## 17. Portfolio Integration

### Where it goes

- **Portfolio page:** `/projects/gear-nest` — rendered from a `Project` entry in `lib/projects/data.ts` (same pattern as MarketMind — TypeScript data object, NOT MDX). `content/projects/` is empty in the portfolio repo.
- **Home page:** Featured project card alongside MarketMind via `ProjectCard` component.

### Portfolio entry to add

Add to `lib/projects/data.ts` in the personal-website repo — insert before the `outbox-kit` entry:

```typescript
{
  slug: 'gear-nest',
  title: 'GearNest',
  tagline: 'Semantic outdoor & fitness gear aggregator — 50k products, AI chat, price comparison across 8 stores',
  tag: 'Product',
  status: 'Building',
  period: '2026 – present',
  liveUrl: 'https://gearnest.io',   // update when live
  summary:
    'GearNest aggregates 50,000+ outdoor, hiking, running, camping, and fitness products across 8 retailers. A daily batch job (Rust) scrapes prices and reviews, normalises them into a canonical catalog, and generates vector embeddings. Users browse a unified product catalog, compare prices ranked by a best-value score (price × store rating), and ask complex natural-language questions ("Is this sleeping bag good below -20°F?") — answered in real-time by a Java RAG orchestrator retrieving grounded context from indexed product specs and community reviews.',
  metrics: [
    { value: '50k+', label: 'Products', context: '8 stores: Amazon, REI, Backcountry, Cabela\'s, Moosejaw, Steep & Cheap, CampSaver, Garage Grown Gear' },
    { value: '~3M', label: 'Review chunks', context: 'indexed in pgvector, exact KNN retrieval per product' },
    { value: '8', label: 'Stores aggregated', context: 'affiliate API + scrapers + headless fallback per store tier' },
    { value: '20+', label: 'ADRs', context: 'entity resolution, HNSW anti-pattern, stratified retrieval, Redis price decoupling' },
  ],
  stack: [
    'Rust (Tokio + reqwest + scraper + sqlx)',
    'Java 21 + Spring Boot 3',
    'Spring AI (HuggingFace)',
    'Next.js 16 (App Router)',
    'TypeScript strict',
    'PostgreSQL 16 + pgvector',
    'Redis (Upstash)',
    'BAAI/bge-small-en-v1.5 (384d embeddings)',
    'Mistral-7B-Instruct (HF Pro)',
    'Tailwind v4 + shadcn/ui',
    'Framer Motion',
    'Docker + Fly.io + Vercel',
  ],
  problem:
    'Outdoor enthusiasts shop across 8+ retailers comparing prices manually, reading scattered reviews on each site, and using keyword search that fails for nuanced queries like "which tent handles 4-season alpine conditions?" Built this because I hike, trail run, and CrossFit — and this tool genuinely should exist.',
  sections: [
    {
      heading: 'Rust for the ingestion pipeline — not Python',
      paragraphs: [
        'Scraping 8 stores concurrently, normalising 50k products, chunking 3M review texts, and generating vector embeddings for all of them is CPU and I/O bound. Rust\'s async model (Tokio) runs all 8 store scrapers concurrently with zero data races guaranteed at compile time. Rayon handles CPU-bound normalisation in parallel. The memory footprint is 10× lower than a JVM equivalent, which matters on constrained Fly.io batch job machines.',
        'The pipeline is designed around a StoreCrawler trait — each store\'s implementation is transport-agnostic (clean HTTP, proxy rotation, or headless browser via chromiumoxide). Upgrading one store\'s anti-bot approach is a one-file change that doesn\'t touch the normaliser, chunker, or embedder.',
      ],
    },
    {
      heading: 'Entity resolution — the hardest correctness problem in e-commerce aggregation',
      paragraphs: [
        '"MSR PocketRocket 2" and "Mountain Safety Research Pocket Rocket II Stove" are the same product. Naive string similarity breaks immediately on real retailer title data. GearNest uses a three-tier resolution system: structural identifiers (GTIN/ASIN, O(1) lookup from affiliate APIs), structured attribute extraction (brand alias table + model number regex, maps "Mountain Safety Research" → "msr"), and embedding similarity on canonical attribute strings as a fallback.',
        'Each match gets a confidence tier: EXACT, HIGH, MEDIUM, or CANDIDATE. CANDIDATE rows are written to the DB but filtered from the user-facing price comparison table — they sit in a review queue rather than silently corrupting the product data. This is documented in ADR-007 as the single most important design invariant in the ingestion pipeline.',
      ],
    },
    {
      heading: 'Java RAG orchestrator — product-scoped chat via stratified retrieval',
      paragraphs: [
        'The AI chat is always product-scoped. Users ask about a specific product ("Is this good for below-freezing conditions?"), and the system retrieves context from that product\'s indexed chunks only. Because the filter is so selective (~75 rows per product), HNSW vector indexes would degrade to sequential scans — exact KNN over 75 embeddings is microseconds and 100% accurate. The pgvector indexes are btree on product_id, not HNSW.',
        'Retrieval is stratified to prevent sentiment bias: semantic top-5 (via MMR for diversity) + top-3 negative review chunks (guaranteed failure modes regardless of query direction) + top-2 spec chunks. A pure top-10 cosine query on "Is this good for rain?" would cluster around rain-related chunks and miss the snow failure. Stratification makes answers trustworthy. The session budget (5 questions per 2-hour session) uses a Redis reserve-then-commit pattern — budget rolls back if HuggingFace never responds, preventing wasted questions on timeouts.',
      ],
    },
  ],
  shipped: [],  // fill in as features land
  links: [
    { label: 'Live site', href: 'https://gearnest.io', primary: true },
  ],
},
```

### Case study narrative (for `sections[]`)

When building out the sections above, use these as the talking point anchors:
1. The Problem (fragmented outdoor gear shopping — personal motivation)
2. Rust ingestion pipeline (concurrency, trait-based scraper design, anti-bot tiers)
3. Entity resolution (three-tier confidence, CANDIDATE quarantine)
4. Why exact KNN beats HNSW for product-scoped RAG
5. Stratified retrieval + MMR (balanced sentiment, failure mode guarantee)
6. Reserve-then-commit session budget (distributed systems pattern applied to UX)

### Interview Talking Points

**"Why Rust for the ingestion pipeline?"**
> "Scraping and embedding 2M+ review chunks is CPU and I/O bound. Rust's async model (Tokio) lets me run 8 store scrapers concurrently with zero data races at compile time. Rayon handles CPU-bound normalization in parallel. The memory footprint is 10x lower than a JVM-based equivalent, which matters when I'm running batch jobs on constrained Fly.io machines."

**"Why Java for the RAG orchestrator?"**
> "Spring Boot 3 with virtual threads handles the SSE streaming cleanly — each chat connection is a virtual thread, no reactive programming overhead. Spring AI gives me a vendor-neutral abstraction over the HuggingFace models. And honestly, Java is what enterprise companies use for their core APIs — I wanted to show I can build production-grade services with it, not just toy microservices."

**"Explain your chunking strategy."**
> "I used two different strategies. For product specs — structured sentences like 'Weight: 14 oz. Insulation: 800-fill' — I used sentence-boundary chunking to preserve attribute-value pairs. For reviews — free-form user text — I used fixed-size 256-token chunks with 32-token overlap. Overlap prevents losing context at chunk boundaries, like when a review's conclusion starts at the end of chunk 1 and spills into chunk 2."

**"How do you prevent hallucination?"**
> "The system prompt explicitly instructs the model to answer only from provided context and say so if the context is insufficient. I also hard-cap retrieved context at 10 chunks and label each chunk as SPEC, POSITIVE_REVIEW, or CRITICAL_REVIEW so the model knows the provenance of what it's citing."

**"Why didn't you use an HNSW index on your vector tables?"**
> "I considered it, but HNSW is a global nearest-neighbor structure. My RAG queries are always product-scoped — WHERE product_id = ? — which eliminates 99.98% of the graph before HNSW can even do anything useful. At that selectivity, HNSW traversal hits dead ends and falls back to a sequential scan that's slower than exact cosine similarity over the ~75 remaining rows. I dropped the HNSW indexes on the chunk tables entirely and added a plain btree on product_id. Faster, simpler, and 100% accurate."

**"How do you handle the entity resolution problem?"**
> "Cross-store product matching is the hardest correctness problem in the pipeline. I built a three-tier resolution system: first check structural identifiers like GTIN or ASIN — that's an O(1) exact match. If those are absent, normalize the brand name through an alias table and extract model tokens with regex — 'Mountain Safety Research' becomes 'msr', 'Pocket Rocket II' becomes 'pocketrocket-2'. Finally, embedding similarity on the normalized canonical string as a fallback. Each match gets a confidence tier: EXACT, HIGH, MEDIUM, or CANDIDATE. Only the first three are shown to users. CANDIDATES sit in a review queue rather than silently polluting the price comparison table."

---

## 18. Key Architecture Decisions (ADRs)

### ADR-001: pgvector over a dedicated vector database
**Decision:** Use PostgreSQL + pgvector extension rather than Pinecone, Weaviate, or Qdrant.  
**Rationale:** Products, reviews, prices, and vectors are all relational. Keeping them in one database eliminates cross-service joins and consistency complexity. Because chat is always product-scoped, we use exact KNN over ~75 rows per product — no HNSW needed on chunk tables, and no dedicated vector DB performance advantage applies at this scope. A dedicated vector DB adds operational overhead with no benefit.  
**Trade-off:** If chat scope ever expands to cross-product queries over 8M+ vectors, add HNSW then — or revisit Qdrant. But YAGNI.

### ADR-002: Rust for ingestion, not Python
**Decision:** Rust handles all scraping, normalization, chunking, and embedding calls.  
**Rationale:** Portfolio signal (demonstrates Rust in a real-world pipeline). Technically justified: concurrent async I/O with Tokio is more memory-efficient than Python async for 8 simultaneous store scrapers. The `scraper` crate provides CSS-selector-based HTML parsing comparable to BeautifulSoup with zero GC overhead.  
**Trade-off:** Rust has a steeper iteration cycle than Python Scrapy. Accepted cost for the portfolio value.

### ADR-003: Session-based LLM limits without auth
**Decision:** 5 questions per 2-hour session tracked via Redis, no user accounts.  
**Rationale:** Auth creates friction that kills casual discovery. HuggingFace Pro rate limits need managing. Cookie sessions are transparent to the user and reset naturally. The limit is shown in the UI with a "why" tooltip.  
**Trade-off:** Users who want unlimited access have no path to it in v1. Accepted — this is about the community, not power users.

### ADR-004: BAAI/bge-small-en-v1.5 for embeddings
**Decision:** 384-dimension embeddings from `BAAI/bge-small-en-v1.5`.  
**Rationale:** Outperforms `all-MiniLM-L6-v2` on MTEB retrieval benchmarks. 384 dimensions vs 1536 (OpenAI ada-002) → 4x smaller vectors → 4x smaller HNSW index → faster search, cheaper storage. Free via HuggingFace Inference API.  
**Trade-off:** Slightly lower quality than OpenAI text-embedding-3-large on complex queries. Acceptable for product/review retrieval.

### ADR-005: Hybrid search (vector + FTS) for catalog
**Decision:** Product catalog search uses weighted combination of pgvector cosine similarity and PostgreSQL full-text search.  
**Rationale:** Semantic search alone ranks "lightweight waterproof jacket" well but struggles with exact brand/model queries ("Arc'teryx Beta AR"). FTS handles exact matches. Weighted combination (0.6 vector + 0.4 FTS) covers both use cases.

### ADR-006: Redis for mutable price data (MVCC decoupling)
**Decision:** Current prices and in-stock flags live in Redis hashes, not as columns on `store_listings`.  
**Rationale:** PostgreSQL MVCC writes a new row version on every UPDATE. Daily price sync across 50,000 listings = 50,000 UPDATE statements = severe table bloat and forced vacuum cycles that degrade frontend read latency. Redis hash (`HSET "prices:{product_id}" store_id {price, stock, ts}`) is append-friendly by nature — no versioning overhead. `price_history` in Postgres remains the append-only source of truth for trends.  
**Trade-off:** Price reads require two-source lookup (Postgres + Redis). Handled transparently in `PricingService` with Redis-miss fallback to `price_history`.

### ADR-007: Three-tier entity resolution with confidence gating
**Decision:** Cross-store product matching uses GTIN/ASIN (Exact) → structured attribute extraction (High) → embedding similarity (Medium/Candidate), with `CANDIDATE` rows excluded from user-facing UI.  
**Rationale:** Naive string similarity on raw product titles produces massive false positives. "MSR PocketRocket 2" ≠ "Mountain Safety Research Pocket Rocket II Stove" under edit-distance metrics, yet they are the same product. Structured attribute normalization (brand alias + model regex) catches the majority deterministically. Confidence gating prevents silent data corruption: uncertain matches go to a review queue, never to the price comparison table.  
**Trade-off:** Tier 2 requires maintaining a brand alias dictionary. ~200 outdoor + fitness brands is manageable. Accuracy on niche cottage brands (Garage Grown Gear exclusives) will be lower — acceptable since those products rarely appear on other stores.

### ADR-008: Stratified + MMR retrieval over pure top-K cosine
**Decision:** RAG retrieval uses semantic top-5 (MMR) + top-3 negative review chunks + top-2 spec chunks, rather than pure top-10 cosine similarity.  
**Rationale:** Pure top-K on a directional query ("Is this good for rain?") clusters retrieved chunks around the query topic and systematically excludes failure modes. A user asking about rain performance deserves to see the 3 reviews saying "failed in snow" — that's what makes the answer trustworthy. Stratified retrieval guarantees balanced sentiment at the cost of two additional filtered queries. MMR on the semantic slots ensures topical diversity rather than returning near-duplicates.  
**Trade-off:** Slightly more complex retrieval logic in `RagService`. Worth it for answer quality and user trust.

---

### ADR-009: Stale-while-revalidate for Redis price cache (no hard TTL)
**Decision:** Redis price keys are permanent (no EXPIRE). Staleness is application-managed via an embedded `fetched_at` timestamp. Reads serve stale data immediately and trigger async background refresh when data is > 24 hours old. First-write jitter (0–60 min) spreads the 24-hour staleness window across the catalog.  
**Rationale:** Hard TTL on 50,000 price keys set by a single batch run creates synchronized expiry — a thundering herd that shifts all price reads onto Postgres simultaneously under morning traffic. Stale-while-revalidate keeps Postgres isolated from Redis misses regardless of pipeline timing. Stale price display is bounded at 25 hours and is shown honestly in the UI.  
**Trade-off:** Application must manage staleness rather than relying on Redis TTL semantics. Slightly more complex PricingService logic. Accepted — the thundering herd risk is a real production failure mode.

### ADR-010: Range-partition price_history by month
**Decision:** `price_history` is range-partitioned on `fetched_at` with monthly child partitions created dynamically by the Rust pipeline.  
**Rationale:** 400k rows/day × 365 days = 146M rows/year. Without partitioning, the B-tree index on `(listing_id, fetched_at DESC)` grows to 100M+ entries, degrading both write performance (daily price sync) and read performance (trend chart queries). Monthly partitioning ensures daily writes target the hot current partition; trend queries hit ≤ 2 partitions; retention policy = `DROP PARTITION` (O(1), no lock). Directly maps to the list-partitioned Postgres pattern from PRISM / cxt-msg-asset-service at Walmart.  
**Trade-off:** Foreign key enforcement is at the application layer (partitioned tables have constraints on FK with non-partitioned parents). Pipeline must create next month's partition before month rollover — handled by idempotent DDL at pipeline startup.

### ADR-011: MinHash Stage 2 gated at 150-char review body length
**Decision:** Stage 2 near-deduplication (MinHash LSH) only runs on reviews with body length ≥ 150 characters.  
**Rationale:** 3-gram word shingles on short reviews ("Great product, fast shipping!") produce near-identical token sets across distinct authentic reviews, causing false-positive deduplication. Stage 2 is designed to catch multi-paragraph syndicated corporate content — not short genuine reviews. Reviews under 150 chars are already handled by `UNIQUE(store_id, source_review_id)` exact dedup.  
**Trade-off:** Short duplicate reviews across stores won't be detected. Acceptable — short cross-store review duplication is rare and preserving authentic short reviews is more important than eliminating the edge case.

### ADR-012: GCP (Cloud Run + Cloud SQL) over Fly.io
**Decision:** Java API runs on Cloud Run; PostgreSQL runs on Cloud SQL. Fly.io dropped. Redis stays on Upstash (not GCP Memorystore).  
**Rationale:** GCP signals enterprise cloud fluency directly relevant to Staff SWE roles at Walmart and peers — IAM/Workload Identity, VPC private networking, Secret Manager, Cloud Monitoring are all patterns used in production at F500 companies. Fly.io is excellent DX but sends a startup/indie signal, not an enterprise one. Memorystore excluded because its minimum instance cost (~$35/month) alone would exceed the $30/month project budget cap; Upstash free tier is functionally equivalent at portfolio scale.  
**Trade-off:** Significantly more setup complexity than Fly.io (IAM policies, VPC connectors, Workload Identity Federation for GitHub Actions). Accepted — the setup work itself is portfolio signal. Estimated monthly cost: $19–20, well under the $30 hard cap enforced by the billing auto-stop Cloud Function (§15.5).

---

## 19. Parallel Implementation Guide (Claude Code CLI)

The current build phases mix all three services within each phase. Running two Claude Code CLI sessions simultaneously against the same working tree will cause merge conflicts, overlapping schema migrations, and file corruption. This section restructures the work into isolated service tracks that can run in parallel without interference.

---

### 19.1 The Problem With the Current Phase Structure

Phase 1 as written includes:
- Rust: Amazon scraper, normalizer, pgvector insert  
- Java: `ProductController`, `SearchService`, `PricingService`  
- Next.js: catalog grid, product detail, search bar  

If two sessions each try to write `src/main.rs` + `src/main/java/...` + `app/` simultaneously, one will overwrite the other. Phases are not the unit of isolation — **services are**.

---

### 19.2 Session Map

| Session | Track | Owns | Never Touch |
|---------|-------|------|-------------|
| **Session 0** | Contract | `docs/` · `supabase/migrations/` · root `docker-compose.yml` · per-service `CLAUDE.md` | Any service `src/` |
| **Session A** | Pipeline | `gear-nest-pipeline/` | `gear-nest-api/` · `gear-nest-web/` |
| **Session B** | API | `gear-nest-api/` | `gear-nest-pipeline/` · `gear-nest-web/` |
| **Session C** | Web | `gear-nest-web/` | `gear-nest-pipeline/` · `gear-nest-api/` |

Session 0 is a **gate** — Sessions A/B/C cannot start until Session 0 has merged into `main`.

---

### 19.3 Contract-First Prerequisite

Before any parallel work begins, Session 0 must lock:

1. **Full SQL schema** (`supabase/migrations/0001_initial_schema.sql`) — every table, index, partition definition from §6 of this spec. No subsequent session may ALTER tables; they add new migration files.  
2. **OpenAPI spec** (`docs/api/openapi.yaml`) — all endpoints from §10. Session B implements to this contract; Session C consumes it. Drift = Session 0 re-approves.  
3. **`docker-compose.yml`** — Postgres 16 + pgvector, Redis, stub Java service, stub Next.js dev. All sessions start the same stack.  
4. **Per-service CLAUDE.md files** — see §19.6 below.  
5. **ADR number reservation** — ADR-012 through ADR-020 pre-allocated in `docs/adr/` as empty stubs so sessions can claim numbers without conflicts.

Session 0 branch: `feat/contract`. Merges to `main` before any A/B/C branch is created.

---

### 19.4 Git Worktree Setup

Each Claude Code CLI session runs in its own git worktree — a separate checkout of the repo rooted in a sibling directory. The session sees only its branch's files; no shared working tree.

```bash
# Run once from the monorepo root after Session 0 merges:
cd ~/Claude\ Projects/gear-nest

# Session A — Pipeline
git worktree add ../gear-nest-pipeline-wt feat/pipeline-phase1

# Session B — API
git worktree add ../gear-nest-api-wt feat/api-phase1

# Session C — Web
git worktree add ../gear-nest-web-wt feat/web-phase1
```

Launch each Claude Code session in its own worktree directory:

```bash
# Terminal 1
cd ~/Claude\ Projects/gear-nest-pipeline-wt && claude

# Terminal 2
cd ~/Claude\ Projects/gear-nest-api-wt && claude

# Terminal 3
cd ~/Claude\ Projects/gear-nest-web-wt && claude
```

Each session opens `CLAUDE.md` from its worktree root. That file enforces scope (§19.6).

To clean up a worktree after merging:
```bash
git worktree remove ../gear-nest-pipeline-wt
```

---

### 19.5 Dependency Graph (Phase 1)

```
Session 0 (contract) ─────────────────────┐
  migrations/0001_initial_schema.sql        │
  docs/api/openapi.yaml                     │
  docker-compose.yml                        ▼
  per-service CLAUDE.md            [merge to main]
                                            │
          ┌─────────────────────────────────┤
          │                     │           │
          ▼                     ▼           ▼
   Session A              Session B    Session C
   Pipeline               Java API     Next.js
   (reads schema)         (reads       (reads OpenAPI,
   writes to DB           schema +     calls Java API)
   via psql)              OpenAPI)
          │                     │
          └──────── both merge ─┘── then Session C
                  to main first      can integrate
```

Session C has a soft dependency on Session B: the frontend can be built against mock data (using the OpenAPI spec as a contract), then swapped to the real API when Session B merges. No blocking wait required.

---

### 19.6 Per-Service CLAUDE.md Content

These files are created by Session 0. Each session's working directory will have one of these as its `CLAUDE.md`.

#### `gear-nest-pipeline/CLAUDE.md`

```markdown
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
- `docs/adr/` — claim your pre-allocated ADR number (ADR-012 to ADR-015); do not renumber

## CONFLICT ZONES
- `CHANGELOG.md` — append-only; add one line at the bottom, never rewrite history
- `README.md` — append to the Pipeline section only; leave API/Web sections untouched

## Coding conventions
- Rust edition 2021, stable toolchain (see `rust-toolchain.toml`)
- Never hardcode DB credentials; read `DATABASE_URL` from environment
- `cargo fmt` + `cargo clippy -- -D warnings` must pass before commit

## Running the stack
```bash
docker compose up -d postgres redis
cargo run -- --help
```
```

---

#### `gear-nest-api/CLAUDE.md`

```markdown
@../AGENTS.md

# GearNest API — Session B Scope Boundaries

## YOU OWN
- Everything under `gear-nest-api/` (Spring Boot service)
- May READ: `supabase/migrations/` (schema reference)
- May READ: `docs/api/openapi.yaml` (implement to this contract; do not modify it without Session 0 sign-off)

## DO NOT TOUCH
- `gear-nest-pipeline/` — any file, any reason
- `gear-nest-web/` — any file, any reason
- `supabase/migrations/` — read-only; propose schema changes to Session 0
- `docs/adr/` — claim pre-allocated ADR-016 to ADR-018; do not renumber

## API CONTRACT DISCIPLINE
- All endpoints must match `docs/api/openapi.yaml` exactly (path, method, request/response shape)
- If a contract change is needed, open a PR comment against Session 0 — do not silently diverge

## CONFLICT ZONES
- `CHANGELOG.md` — append-only at bottom
- `README.md` — append to the API section only

## Running the stack
```bash
docker compose up -d postgres redis
./mvnw spring-boot:run
```
```

---

#### `gear-nest-web/CLAUDE.md`

```markdown
@../AGENTS.md

# GearNest Web — Session C Scope Boundaries

## YOU OWN
- Everything under `gear-nest-web/` (Next.js app)
- May READ: `docs/api/openapi.yaml` (for API types and mock data shape)

## DO NOT TOUCH
- `gear-nest-pipeline/` — any file, any reason
- `gear-nest-api/` — any file, any reason
- `supabase/migrations/` — any file, any reason
- `docs/adr/` — claim pre-allocated ADR-019 to ADR-020; do not renumber

## CODING CONVENTIONS (must match portfolio-website)
- Next.js 16.x, TypeScript strict (no `any`, no `@ts-ignore`)
- Tailwind v4 — dark mode via `dark:` prefix + `@custom-variant dark` in globals.css
- Server Components by default; `'use client'` only for browser APIs / event handlers / hooks
- `@/` alias for all imports
- `cn()` from `@/lib/utils` for conditional classes — never string concatenation

## MOCK DATA DURING API DEVELOPMENT
Until Session B merges, consume `lib/mock/products.ts` (generated from OpenAPI spec).
Swap for real API calls in `lib/api/` — the mock and real client must have identical TypeScript interfaces.

## CONFLICT ZONES
- `CHANGELOG.md` — append-only at bottom
- `README.md` — append to the Web section only

## Before committing
```bash
npm run typecheck && npm run lint && npm run build
```
```

---

### 19.7 Shared Conflict Zones & Resolution Rules

| File | Owner | Rule |
|------|-------|------|
| `CHANGELOG.md` | Append-only | Every session appends one line at the bottom. Never rewrite or reorder. Format: `YYYY-MM-DD · <service> · <feature> — one-line summary` |
| `README.md` | Sectioned | Each service has its own H2 section. Sessions edit only their section. |
| `supabase/migrations/` | Session 0 only | No session may add migration files without Session 0 review. Schema changes proposed via PR comment. |
| `docker-compose.yml` | Session 0 only | If a new service dependency is needed (e.g., a test container), request via PR comment. |
| `docs/adr/` | Pre-allocated | ADR numbers reserved: 012-015 (Pipeline), 016-018 (API), 019-020 (Web). Sessions write content, never renumber. |

---

### 19.8 Phase 1 Restructured as Parallel Tracks

**Session 0 — Contract (must complete first, ~1 day)**

- [ ] Full SQL schema migration: `supabase/migrations/0001_initial_schema.sql`
- [ ] OpenAPI spec: `docs/api/openapi.yaml` (all Phase 1 endpoints)
- [ ] `docker-compose.yml` with Postgres 16 + pgvector extension, Redis, placeholder services
- [ ] `gear-nest-pipeline/CLAUDE.md`, `gear-nest-api/CLAUDE.md`, `gear-nest-web/CLAUDE.md`
- [ ] `docs/adr/` stubs for ADR-012 through ADR-020
- [ ] Root `CLAUDE.md` with monorepo-level conventions
- [ ] Merge to `main`; create the three git worktrees

**Session A — Pipeline Track (after Session 0 merges)**

- [ ] Rust workspace: `Cargo.toml`, `rust-toolchain.toml`, workspace members
- [ ] `gear-nest-pipeline/src/db/` — Postgres connection pool (`sqlx`), migration runner
- [ ] `gear-nest-pipeline/src/scrapers/amazon.rs` — PA API client, product fetch, raw JSON storage
- [ ] `gear-nest-pipeline/src/normalizer/` — title normalization, category mapping, brand alias table
- [ ] `gear-nest-pipeline/src/entity_resolution/` — Tier 1 (GTIN/ASIN), Tier 2 (structured), Tier 3 skeleton
- [ ] `gear-nest-pipeline/src/embeddings/` — HuggingFace Inference API client, batch embed, pgvector insert
- [ ] `gear-nest-pipeline/src/prices/` — stale-while-revalidate Redis writer with jitter
- [ ] `gear-nest-pipeline/src/price_history/` — partition-aware append, idempotent DDL at startup
- [ ] Integration test: scrape 50 Amazon products → normalize → embed → assert in DB

**Session B — API Track (after Session 0 merges)**

- [ ] Spring Boot project scaffold: `pom.xml`, `application.yml`, `docker-compose` dev profile
- [ ] `ProductController` — `GET /api/products`, `GET /api/products/{id}`, `GET /api/products/search`
- [ ] `SearchService` — hybrid search (pgvector + FTS, 0.6/0.4 weighting)
- [ ] `PricingService` — Redis hash read, `price_history` fallback, stale indicator
- [ ] `BestValueScorer` — `0.4×price_score + 0.6×rating_score`
- [ ] `SessionBudgetService` — Redis reserve-then-commit, DECR + inflight EX 90
- [ ] `RagController` + `RagService` — SSE streaming, stratified retrieval, HuggingFace Inference client
- [ ] Integration test: POST `/api/chat` with seeded product → assert SSE stream contains product name

**Session C — Web Track (after Session 0 merges; mock data until Session B merges)**

- [ ] Next.js app scaffold: `gear-nest-web/`, Tailwind v4, `@/` alias, dark mode wired
- [ ] `lib/mock/products.ts` — typed mock data matching OpenAPI schema
- [ ] `app/(catalog)/page.tsx` — catalog grid, facet sidebar (category, price range, rating)
- [ ] `app/(catalog)/[slug]/page.tsx` — product detail: specs, price comparison table, review samples
- [ ] `components/search/SearchBar.tsx` — debounced, semantic search (Server Action → API)
- [ ] `components/chat/ChatPanel.tsx` — SSE consumer, streaming message display, session budget indicator
- [ ] `components/prices/PriceTable.tsx` — per-store price rows, stale badge, best-value highlight
- [ ] Swap mock data for real API calls once Session B merges
- [ ] `npm run typecheck && npm run lint && npm run build` — must pass

---

### 19.9 Merge Order & Integration

```
Session 0 (contract) → main
        ↓
Session A (pipeline) → PR → main   ← can merge any time after Session 0
Session B (api)      → PR → main   ← can merge any time after Session 0
        ↓ (both merged)
Session C (web)      → swap mocks → final integration PR → main
```

Session C's final PR is the integration milestone: real scraper data + real API + real frontend. This is when Phase 1 is complete.

---

### 19.10 Starting a New Session Mid-Phase

When a new Claude Code CLI session opens in a worktree:

1. Read `CLAUDE.md` in the worktree root — this defines scope.  
2. Read `SPEC.md` §16 (Build Phases) and §19 (this section) — understand where you are in the plan.  
3. Read `CHANGELOG.md` — last few entries tell you what has shipped.  
4. Run `git log --oneline -10` on the current branch — your recent commits.  
5. Pick up the next unchecked item in §19.8 for your track.

Never ask "what should I work on?" — the unchecked items in your track's list are the answer.

---

*Spec v1.2 complete. All architectural concerns and validation questions addressed. Ready to begin Phase 1.*
