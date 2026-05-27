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

## Coding conventions
- Java 21, records and sealed types where they help
- Virtual threads via `spring.threads.virtual.enabled=true`; spawn long work on `Thread.ofVirtual()`
- Spring AI for HuggingFace embedding + chat models
- No comments unless WHY is non-obvious
- `./mvnw test` must pass before commit

## Running the stack
```bash
docker compose up -d postgres redis
./mvnw spring-boot:run
```

## Phase 1 Track (SPEC §19.8)
- [ ] Spring Boot project scaffold: `pom.xml`, `application.yml`, `docker-compose` dev profile
- [ ] `ProductController` — `GET /api/v1/products/search`, `/api/v1/products/{slug}`
- [ ] `SearchService` — hybrid search (pgvector + FTS, 0.6/0.4 weighting)
- [ ] `PricingService` — Redis hash read, `price_history` fallback, stale indicator
- [ ] `BestValueScorer` — `0.4×price_score + 0.6×rating_score`
- [ ] `SessionBudgetService` — Redis reserve-then-commit, DECR + inflight EX 90
- [ ] `RagController` + `RagService` — SSE streaming, stratified retrieval, HuggingFace client
- [ ] Integration test: GET `/api/v1/chat` with seeded product → assert SSE stream contains product name
