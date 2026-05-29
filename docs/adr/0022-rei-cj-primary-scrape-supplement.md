# ADR-022: REI = CJ affiliate feed primary, scrape fills the blanks

**Status:** Accepted
**Date:** 2026-05-28
**Owner:** Session A (Pipeline)

> Numbering note: the pre-allocated pipeline range (013–015) was used by the
> Phase-2 transport-tier ADRs, so this pipeline ADR takes the next free number
> (021 is the contract-owned Redis-schema ADR).

## Decision

REI is the only "API + scrape" store (SPEC §7). Its catalog is fetched from the
CJ (Commission Junction) affiliate GraphQL Product Search API; CJ omits
structured specs, long descriptions, and reviews, so each CJ record is
supplemented by a clean-HTTP scrape of the REI product page (the shared
`scrapers::jsonld` parser). The two are merged with a fixed precedence
(`rei::merge_supplement`):

- **CJ is authoritative** for the commerce + identity fields: `price` /
  `in_stock` (CJ's `salePrice` preferred over list `price`), `gtin`,
  `store_product_id`, `url`, `title`, `brand`, and `raw_payload`.
- **The scrape only fills blanks**: `description`, `category_path`, `features`,
  `specs`, `primary_image`, and `store_rating` / `store_review_count` are taken
  from the scrape *only when CJ left them empty*.

## Rationale

CJ is the contractually correct source for price and the affiliate link (that is
what we are paid on and what must match the click destination), and it reliably
carries GTINs — which drive Tier-1 entity resolution (ADR-007). But CJ's feed is
commerce-shaped and thin on the editorial content the RAG layer needs. Scraping
the product page recovers that content while reusing the exact same JSON-LD
parser as every other store, so REI adds no new parsing surface — only the
discovery source (CJ) and the merge are REI-specific. A scrape failure
degrades gracefully: the CJ record stands on its own (price + GTIN + link are
all present), so REI never blocks on the supplement.

## Trade-off

Two fetches per product (CJ + page) make REI the slowest non-headless store, and
the precedence is a deliberate guess: if CJ's title/category were ever worse than
the page's, we would still keep CJ's. That is acceptable because the commerce
fields are exactly where CJ must win, and the entity resolver canonicalizes
titles downstream regardless. If CJ ever starts returning specs/descriptions,
the supplement scrape becomes redundant and can be dropped without touching the
merge contract.
