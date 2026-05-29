# ADR-014: Proxy tier delegates IP rotation to the provider, not in-process

**Status:** Accepted
**Date:** 2026-05-28
**Owner:** Session A (Pipeline)

## Decision

Backcountry, Moosejaw, and Steep & Cheap sit behind enterprise bot protection
(Akamai / Cloudflare) and are assigned `Tier::Proxy`. Two boundaries:

1. **Rotation is the provider's job.** Each store routes through a single
   `SCRAPE_PROXY_{STORE}` endpoint (a residential proxy provider such as Bright
   Data / Zyte that rotates exit IPs on its side). The pipeline does **not**
   manage a proxy pool, health-check IPs, or rotate endpoints itself. If
   `SCRAPE_PROXY_{STORE}` is unset, the store falls back to direct HTTP.

2. **Proxy is a transport-only concern.** These stores are Salesforce Commerce
   Cloud sites that emit the same schema.org JSON-LD as the clean-HTTP stores,
   so they reuse the shared `scrapers::jsonld` parser unchanged — only the
   `Transport` differs. No proxy-specific parsing or per-store code beyond the
   tier selection and URL conventions (ADR-013).

Per-store courtesy rate limits (`governor`) and 429/503 backoff live in the
price-sync job (PR6, SPEC §7), not here.

## Rationale

Residential-proxy vendors already solve IP reputation and rotation far better
than anything we could build, and their APIs expose it as a single endpoint.
Re-implementing rotation in-process would duplicate that, add stateful failure
modes, and still be inferior. Keeping the JSON-LD parser shared across clean-HTTP
and proxy tiers means adding a proxy store is the same one-file change as a
clean-HTTP store, just with `Tier::Proxy`.

## Trade-off

We are coupled to the proxy provider's reliability and bill, with no in-process
fallback rotation if a provider degrades — acceptable because the daily
price-sync tolerates a store being briefly unreachable (stale-while-revalidate,
ADR-009) and a provider swap is a credential change, not code. If a store later
needs JavaScript execution to render prices, it moves to the headless tier
(ADR-015), not a richer proxy layer.
