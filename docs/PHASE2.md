# GearNest Phase 2 — Multi-Store + Price Comparison

> Takes GearNest from 1 store (Amazon) to 8. SPEC §16 Phase 2 / §7 transport tiers.
> Predominantly **pipeline (Session A)** work + one **API** task. Read alongside
> SPEC.md §7 and the per-service `CLAUDE.md` scope rules.

## Goal

Live price comparison across all 8 stores, with cross-store products correctly
resolved into one canonical product (ADR-007). Amazon (PA-API) already works;
this phase adds the other 7 and the price-sync that feeds the comparison table
(the API `/prices` endpoint + web `PriceTable` already support multi-store).

## Stores & transport tiers (SPEC §7)

| Store | Tier | Method |
|-------|------|--------|
| Amazon | API | PA-API (done) |
| REI | API+scrape | CJ Affiliate API + scrape supplement |
| Garage Grown Gear, CampSaver | Clean HTTP | `reqwest` + browser headers + cookie jar |
| Backcountry, Moosejaw, Steep & Cheap | Proxy rotation | residential proxy via `SCRAPE_PROXY_{STORE}` |
| Cabela's | Headless | `chromiumoxide` browser pool (single browser, semaphore=3 tabs) |

## Design: extend the `StoreCrawler` trait

Amazon fetches by known ASIN (`fetch_batch(ids)`). Scrape stores have **no ID list** —
they discover products by crawling category pages. Extend the trait:

```rust
async fn crawl_products(&self, category: &Category) -> Result<Vec<RawProduct>>;
async fn fetch_price(&self, store_product_id: &str) -> Result<PriceUpdate>;
```

Keep `fetch_batch` for API stores. Add a transport abstraction so a store's tier
(clean HTTP / proxy / headless) is selectable without touching the
normalizer/resolver/embedder — "upgrading a store from HTTP to headless is a
one-file change" (SPEC §7). Claim **ADR-013** for this trait/transport decision.

## Live scraping vs CI (the reconciliation)

Scrapers hit **real retailer sites at runtime** (real ingestion). But CI can't
gate on live sites (non-deterministic, anti-bot, breakage). So:

- **Runtime**: live HTTP / proxy / headless against the real site.
- **Tests**: capture one **real** product page per store → commit as
  `tests/fixtures/<store>_product.html` → parser unit test asserts extracted
  fields against it. Deterministic, offline, CI-green. Re-capture when a site's
  markup changes (a fixture refresh = a small PR).
- Proxy-tier **live** runs need `SCRAPE_PROXY_{STORE}` creds; the parser tests do not.

## PR breakdown (each is its own CI-gated PR)

1. **Trait + transport foundation** — extend `StoreCrawler` (`crawl_products`,
   `fetch_price`); transport-tier abstraction; **CampSaver** as the reference
   clean-HTTP scraper with a captured-HTML parser test. (ADR-013)
2. **Garage Grown Gear** (clean HTTP) — parser + fixture test.
3. **Backcountry, Moosejaw, Steep & Cheap** (proxy tier) — parser + fixtures;
   proxy transport wired via env. (ADR-014 if a proxy-strategy decision is worth recording)
4. **Cabela's** (headless) — `chromiumoxide` browser pool (SPEC §7), fixture-backed. (ADR-015)
5. **REI** (CJ affiliate) — CJ API client + scrape supplement.
6. **Daily price-sync, all 8 stores** — wire `price_sync` cron
   (`tokio-cron-scheduler`) → Redis SWR writer (exists) + `price_history` append
   (exists). Per-store `governor` rate limits (SPEC §7).
7. **CANDIDATE admin endpoints** (API track — Session B scope) —
   `GET /api/admin/candidates`, `POST .../confirm`, `POST .../reassign` (SPEC §7).
   Surfaces low-confidence entity-resolution matches for review.

Entity resolution (the 3-tier matcher) is finally exercised across stores here —
the CANDIDATE quarantine (ADR-007) becomes load-bearing.

## Definition of done

- All 8 stores ingest products; cross-store duplicates resolve to one product
  (or land in the CANDIDATE queue, never silently merged wrong).
- A product page shows prices from multiple stores, ranked by best-value score.
- `cargo test` (incl. per-store parser fixtures) + clippy + fmt green in CI.
- CHANGELOG line per shipped artifact; ADRs 013-015 filled as decisions land.
