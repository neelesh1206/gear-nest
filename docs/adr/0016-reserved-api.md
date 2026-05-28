# ADR-0016: Redis price schema is a pinned cross-service contract

**Status:** Accepted
**Date:** 2026-05-27
**Owner:** Session B (API) — claimed during Phase 1 integration

## Context
Phase 1 integration testing surfaced a silent bug: the pipeline wrote prices to
Redis as `prices:{product_id}` (hash field = `store_id`, value = JSON), matching
SPEC §6-7, while the API read `price:listing:{listing_id}` with flat hash fields.
Every API price lookup missed Redis, fell back to `price_history`, and marked all
listings stale — defeating ADR-006 / ADR-009. Neither PR review caught it because
each service was reviewed against its own spec interpretation; the Redis schema
was never a contract artifact, only prose.

## Decision
The Redis schema is now pinned in `docs/contracts/redis-schema.md` as the single
source of truth, owned by Session 0 (contract track). The API was corrected to
read the pipeline's format. A contract test (`PricingRedisContractTest`) seeds a
price in the canonical format and asserts the API reads it live.

## Rationale
The relational schema and HTTP API were pinned as contract artifacts in Session 0,
but the Redis schema — equally a cross-service interface — was not. Any data
interface crossing service boundaries must be a reviewable artifact, not prose.

## Trade-off
Redis schema changes now require a contract-track PR plus coordinated writer/reader
updates, rather than either service changing unilaterally. This is the intended
friction — it's the exact safeguard whose absence caused this bug.
