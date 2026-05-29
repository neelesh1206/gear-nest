# ADR-0023: REI — CJ affiliate authoritative; scrape fills blanks only

**Status:** Accepted
**Date:** 2026-05-29
**Owner:** Session 0 (contract track)

## Context
REI has no public product API, but it's in the CJ (Commission Junction) affiliate
network. Phase 2's REI scraper combines two sources per product: the **CJ GraphQL
Product Search** (authoritative affiliate feed) and a **clean-HTTP scrape** of the
product page (richer editorial fields CJ omits). Their fields overlap and can
disagree, so a precedence rule is needed. The REI PR (#16) first recorded this as
`docs/adr/0022`, but 0022 is the Contract/Session-0 range — PR #17 backed it out
and routed the decision to Session 0 to record here.

## Decision
On merge (`rei::merge_supplement`), **CJ is authoritative for the commerce-critical
fields** — price, in-stock, GTIN, store product id, URL, title. The **scrape only
fills fields CJ left blank** — description, features, specs, category, image,
rating.

## Rationale
CJ is the **paid-on** path: affiliate commission requires the outbound link and
price to match the CJ click destination. If a scraped price/link diverged from
CJ's, GearNest could show a price the affiliate link doesn't honor (broken trust)
or forfeit commission. So CJ stays canonical for anything that drives the click or
the money; the scrape is purely additive for editorial richness CJ doesn't carry.

## Trade-off
A scraped price may momentarily be fresher than CJ's feed, but consistency with the
paid link outweighs marginal freshness. The daily price-sync (ADR-0022) re-pulls CJ
prices, bounding staleness. Applies only to REI's dual-source case; single-source
stores (scrape-only or API-only) don't hit this merge path.
