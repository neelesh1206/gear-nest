# ADR-013: Extend `StoreCrawler` for discovery + a tiered transport abstraction

**Status:** Accepted
**Date:** 2026-05-28
**Owner:** Session A (Pipeline)

## Decision

Phase 2 adds 7 scrape stores to the Amazon-only pipeline. Two changes to the scraper layer:

1. **Extend the `StoreCrawler` trait.** Amazon fetches by a known ASIN list
   (`fetch_batch`). Scrape stores have no ID list — they discover products by
   crawling category pages. Add `crawl_products(&Category)` for discovery and
   `fetch_price(&store_product_id)` for the daily price refresh, keeping
   `fetch_batch` for API stores. All three are default methods that `bail!` with
   a "not supported by this store" error; a store overrides only the methods its
   tier supports.

2. **Introduce a `Tier`-selected `Transport` abstraction.** A store names its
   anti-bot tier (`CleanHttp` / `Proxy` / `Headless`); `Tier::transport()`
   returns a `Box<dyn Transport>`. Clean HTTP and proxy share one browser-shaped
   `reqwest` client (cookie jar + browser headers, optional `SCRAPE_PROXY_{STORE}`
   proxy); headless (`chromiumoxide`) lands in PR4 (ADR-015). Parsing and
   normalization depend only on the `Transport::get` -> `String` seam.

CampSaver is the reference clean-HTTP implementation: product fields are parsed
from schema.org JSON-LD, and the pure `parse_*` functions are tested offline
against a committed `tests/fixtures/campsaver_product.html`.

## Rationale

Trait dispatch keeps transport details out of the normalizer / resolver /
embedder, so "upgrading a store from HTTP to headless is a one-file change"
(SPEC §7). Default trait methods avoid forcing every store to stub methods its
tier can never use, while still failing loudly on a mis-dispatch. Parsing from
JSON-LD rather than DOM selectors survives cosmetic markup changes and yields a
deterministic, network-free parser test — CI gates on the fixture, the live site
is hit only at runtime (the live/CI reconciliation in `docs/PHASE2.md`).

## Trade-off

Default-erroring trait methods trade compile-time guarantees (a split trait per
tier would force correct dispatch) for a single `dyn StoreCrawler` the scheduler
can fan out uniformly; mis-dispatch surfaces at runtime, not compile time.
JSON-LD-first parsing breaks for stores that omit structured data — those will
need a DOM or headless fallback per store, recorded when they land.
