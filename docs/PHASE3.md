# GearNest Phase 3 — Reviews

> Aggregated reviews across all 8 stores, with cross-store deduplication and
> per-tier sample selection for the UI. SPEC §16 Phase 3 / §13. Mostly
> **pipeline (Session A)** work, plus one **API** task (Session B) and one
> **web** task (Session C). Read alongside SPEC.md §13 and the per-service
> `CLAUDE.md` scope rules.

## Goal

Every product page shows real reviews from every store that carries it:
total volume, weighted aggregate rating, and 2 surfaced reviews per star
tier. Cross-store duplicates (same reviewer on Amazon and REI) collapse to
one — without a flood of false positives on short reviews (ADR-011).

The `reviews` table and `review_chunks` already exist (`0001_initial_schema.sql`).
Phase 3 fills them; review *embeddings* (chunking + pgvector) are Phase 4.

## Stores & where reviews live

| Store | Source | Notes |
|-------|--------|-------|
| Amazon | PA-API + scrape | PA-API caps at 10 reviews/product; supplement from the product page. |
| REI | Scrape | CJ Affiliate has no reviews endpoint — clean-HTTP scrape of `/product/<id>` reviews tab. |
| Garage Grown Gear, CampSaver | Clean HTTP | Reuse Phase-2 `Transport::Http`. |
| Backcountry, Moosejaw, Steep & Cheap | Proxy | SFCC sites; reviews typically rendered by Bazaarvoice/PowerReviews — different parser than the JSON-LD product parser. |
| Cabela's | Headless | Bazaarvoice widget renders client-side; `Tier::Headless` already handles this. |

## Design: extend the `StoreCrawler` trait

Add one defaulted method, mirroring how `crawl_products` and `fetch_price`
were added in Phase 2:

```rust
async fn fetch_reviews(&self, store_product_id: &str, max: usize) -> Result<Vec<RawReview>>;
```

Implementations paginate internally (sites cap pages 10–25 reviews each)
and stop at `max` (default `500` per SPEC §13). `RawReview` carries
`source_review_id`, `reviewer_id_hash`, `rating`, `title`, `body`,
`verified_purchase`, `helpful_votes`, `review_date`. `reviewer_id_hash`
is `SHA-256(lowercase(store_id + ":" + reviewer_display_name))` when the
site exposes a stable reviewer identifier, else `NULL`.

Persistence is idempotent on `UNIQUE(store_id, source_review_id)` —
`ON CONFLICT DO UPDATE SET helpful_votes = EXCLUDED.helpful_votes` so
re-runs refresh vote counts without inserting dupes.

## Cross-store dedup (Stage 1 only)

SPEC §13 splits dedup into two stages. **Phase 3 ships Stage 1 only.**
Stage 2 (MinHash LSH for anonymous long-form reviews) is explicitly
deferred to Phase 4 per SPEC §16; that's where it pairs with the
embedding pipeline.

Stage 1 is two passes:

1. **Same-store** — handled by the existing `UNIQUE(store_id, source_review_id)` constraint.
2. **Cross-store** — for each `reviewer_id_hash` group with rows from
   ≥2 stores, keep the row with `verified_purchase = true` (tiebreak:
   `helpful_votes DESC`, then `review_date DESC`); delete the others.
   `reviewer_id_hash IS NULL` rows are untouched (Stage 2's job).

DELETE rather than soft-delete: no API or downstream consumer needs the
loser row, and the audit trail lives in the per-store scrape payload
already persisted by `record_raw` (Phase 2). No schema migration.

## Live scraping vs CI (the reconciliation)

Same pattern as Phase 2:

- **Runtime**: live HTTP / proxy / headless against the real review pages.
- **Tests**: capture one **real** reviews page per store →
  `tests/fixtures/<store>_reviews.html` (or `.json` for Bazaarvoice
  responses) → parser unit test asserts extracted fields. Deterministic,
  offline, CI-green.
- Proxy-tier live runs need `SCRAPE_PROXY_{STORE}` creds; the parser
  tests do not.

## PR breakdown (each is its own CI-gated PR)

1. **Trait + reference scraper** — `StoreCrawler::fetch_reviews`, `RawReview`,
   idempotent persistence helper (`reviews::upsert_batch`). **CampSaver**
   as the reference clean-HTTP review scraper with a captured-HTML parser
   test. (No ADR.)
2. **Garage Grown Gear** (clean HTTP) — parser + fixture test.
3. **Backcountry, Moosejaw, Steep & Cheap** (proxy tier) — shared
   Bazaarvoice/PowerReviews parser if applicable; parser + fixtures per store.
4. **Cabela's** (headless) — reviews widget fetched via the existing
   `Tier::Headless` browser pool; fixture-backed test against a
   captured post-render DOM.
5. **REI** (scrape, no CJ) — clean-HTTP scrape of the reviews tab on
   `rei.com/product/<id>`; CJ Affiliate doesn't expose reviews.
6. **Amazon** — PA-API `Reviews` resource (≤10) + product-page scrape
   supplement; merge by `source_review_id`.
7. **`dedup-reviews` one-shot subcommand** — Stage 1 cross-store
   `reviewer_id_hash` pass + a `sync-reviews` subcommand that iterates
   active listings and calls each store's `fetch_reviews`. Scheduled
   externally (Cloud Scheduler → one-shot Cloud Run Job) per ADR-0022,
   wired in Phase 5. Per-store `governor` rate limits (SPEC §7) reused.
8. **API: `GET /api/v1/products/{id}/reviews`** (Session B) — returns
   total counts per star tier + 2 surfaced reviews per tier (verified
   first, helpful_votes DESC) + weighted aggregate rating (SPEC §13).
   `openapi.yaml` first; controller + Testcontainers test.
9. **Web: `ReviewTierSection`** (Session C) — per-tier accordion with
   "X verified of Y total" header and the surfaced sample reviews.

## ADR allocation

Pipeline's pre-allocated block (013–015) was fully consumed by Phase 2.
**No new ADRs anticipated** for Phase 3 — pagination, dedup, and review
chunking are all directly specified in SPEC §13 / ADR-011. If a Phase 3
PR uncovers a decision worth recording, the PR author requests an ADR
number from Session 0 (next available is ADR-024).

## Definition of done

- All 8 stores fetch reviews; the `reviews` table holds real rows for
  every active product × store pair.
- Stage 1 cross-store dedup runs cleanly: same reviewer across Amazon
  and REI collapses to one row, short reviews are untouched.
- A product page shows total review count, aggregate rating, and 2
  reviews per star tier.
- `cargo test` (incl. per-store reviews-parser fixtures) + clippy + fmt
  green in CI; API Testcontainers test green.
- CHANGELOG line per shipped artifact.
