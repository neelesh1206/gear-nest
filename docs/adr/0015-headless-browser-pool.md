# ADR-015: Headless tier — one `chromiumoxide` browser with a tab pool

**Status:** Accepted
**Date:** 2026-05-28
**Owner:** Session A (Pipeline)

## Decision

Cabela's gates product data behind JavaScript and aggressive bot protection, so
it is assigned `Tier::Headless`, implemented with **`chromiumoxide`** (a Rust
Chrome DevTools Protocol client). Specifics:

- **One long-lived browser, many tabs.** `HeadlessTransport` holds a single
  `Browser` (lazily launched on first `get`) and a `Semaphore` capping
  concurrent tabs at 3 (SPEC §7). Each `get` opens a tab, waits for navigation,
  snapshots the rendered DOM (`page.content()`), and closes the tab. The browser
  lock is held only long enough to open the tab, so navigations render in
  parallel up to the semaphore limit.
- **Rendered DOM reuses the shared parser.** The point of headless is to obtain
  the rendered HTML; once we have it, the schema.org JSON-LD is extracted by the
  same `scrapers::jsonld` parser as every other store. No Cabela's-specific
  parsing.
- **Version.** `chromiumoxide = "0.7"` with `default-features = false,
  features = ["tokio-runtime"]` — the latest release that still exposes the
  `tokio-runtime` feature (0.8+ renamed it; 0.9 would pull a second async
  runtime into a tokio project). Defaults are off to keep `async-std` out of the
  tree.

A Chrome/Chromium binary must exist at runtime; `chromiumoxide` auto-discovers
it. Provisioning it in the deployment image is a deploy concern, not part of
this PR (the offline parser test needs no browser).

## Rationale

Running one Chrome per URL exhausts RAM (100–300 MB each); a single browser with
a 3-tab semaphore keeps peak memory at a few hundred MB regardless of fan-out,
and Cabela's runs last in the daily pipeline (SPEC §7) so the browser is only up
briefly. Reusing the JSON-LD parser means the most expensive tier adds no new
parsing surface — it is a transport swap (ADR-013), consistent with the proxy
tier (ADR-014).

## Trade-off

`chromiumoxide` + a Chrome runtime is a heavy dependency (long compile, large
image, an external binary that can crash or hang) — accepted only because
Cabela's genuinely cannot be read over plain HTTP. The browser pool cannot be
exercised in CI without launching Chrome, so CI coverage stays at the parser
level (fixture); the live pool is validated only on real runs. If a future store
needs headless too, it selects `Tier::Headless` — a one-line change — and shares
this one browser pool.
