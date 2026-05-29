#!/usr/bin/env bash
#
# Phase-2 integration smoke test (manual, live).
#
# This is the executable definition of "Phase 2 done" (docs/PHASE2.md):
#   live multi-store ingestion -> cross-store entity resolution (ADR-007)
#   -> 8-store price comparison fed from Redis (docs/contracts/redis-schema.md).
#
# It is NOT a CI gate: it hits real retailer sites and needs live creds, so it
# is non-deterministic by design. CI keeps gating on the per-store captured-HTML
# parser fixtures (deterministic). This harness is what you run by hand to prove
# the seams actually connect on real data.
#
# Stack is driven entirely through docker-compose (Colima locally), so no host
# Rust/Java toolchain is required.
#
# Usage:
#   scripts/phase2_smoke.sh
#
# Optional env:
#   SMOKE_ASINS="B07X... B08Y..."   ASINs for the Amazon single-store stage
#                                    (default: a small built-in set)
#   SMOKE_KEEP=1                     leave the stack running on exit (default: down)
#
# Creds (read from .env.local via docker-compose env_file; all optional):
#   PAAPI_ACCESS_KEY / PAAPI_SECRET_KEY / PAAPI_PARTNER_TAG  -> Amazon stage
#   SCRAPE_PROXY_*                                           -> proxy-tier stores
#   HUGGINGFACE_API_KEY                                      -> spec embeddings
#
set -uo pipefail

cd "$(dirname "$0")/.."

PSQL=(docker compose exec -T postgres psql -U gearnest -d gearnest -tAc)
REDIS=(docker compose exec -T redis redis-cli)
PIPELINE=(docker compose run --rm pipeline)

bold=$(printf '\033[1m'); red=$(printf '\033[31m'); grn=$(printf '\033[32m')
ylw=$(printf '\033[33m'); rst=$(printf '\033[0m')

fail_count=0; skip_count=0; pass_count=0
pass() { echo "${grn}PASS${rst} $*"; pass_count=$((pass_count + 1)); }
skip() { echo "${ylw}SKIP${rst} $*"; skip_count=$((skip_count + 1)); }
fail() { echo "${red}FAIL${rst} $*"; fail_count=$((fail_count + 1)); }
stage() { echo; echo "${bold}== $* ==${rst}"; }

cleanup() {
  if [[ "${SMOKE_KEEP:-0}" == "1" ]]; then
    echo; echo "${ylw}SMOKE_KEEP=1 — leaving stack up. 'docker compose down -v' to tear down.${rst}"
  else
    echo; echo "Tearing down stack…"
    docker compose --profile pipeline --profile api down -v >/dev/null 2>&1 || true
  fi
}
trap cleanup EXIT

sql() { "${PSQL[@]}" "$1" 2>/dev/null | tr -d '[:space:]'; }

# ---------------------------------------------------------------------------
stage "Stage 0 — preconditions"
if ! docker info >/dev/null 2>&1; then
  fail "Docker daemon unreachable. Start it with 'colima start'."
  exit 1
fi
pass "Docker reachable"
[[ -f .env.local ]] && pass ".env.local present (creds will load)" \
                    || skip ".env.local missing — cred-gated stages will SKIP"

# ---------------------------------------------------------------------------
stage "Stage 1 — bring up Postgres + Redis"
docker compose up -d postgres redis >/dev/null 2>&1
for _ in $(seq 1 30); do
  [[ "$(docker compose ps --format '{{.Health}}' postgres 2>/dev/null)" == "healthy" \
   && "$(docker compose ps --format '{{.Health}}' redis 2>/dev/null)" == "healthy" ]] && break
  sleep 2
done
if [[ "$(docker compose ps --format '{{.Health}}' postgres 2>/dev/null)" == "healthy" ]]; then
  pass "postgres + redis healthy"
else
  fail "postgres/redis did not become healthy"; exit 1
fi

# ---------------------------------------------------------------------------
stage "Stage 2 — schema present"
# Postgres applies supabase/migrations via its /docker-entrypoint-initdb.d mount
# on first init, so the schema is already there. We do NOT run `pipeline migrate`
# here: that runner tracks applied versions in `_gn_migrations`, which the initdb
# path never populates, so it would re-run 0001 and fail on "already exists".
tables=$(sql "SELECT count(*) FROM information_schema.tables WHERE table_name IN ('products','store_listings','price_history');")
[[ "$tables" == "3" ]] && pass "schema present (products, store_listings, price_history)" \
                       || { fail "expected 3 core tables, found ${tables:-0} — initdb migration did not run"; exit 1; }

# ---------------------------------------------------------------------------
stage "Stage 3 — single-store ingest (Amazon, the only wired full-pipeline path)"
if grep -q 'PAAPI_ACCESS_KEY=.\+' .env.local 2>/dev/null; then
  asins="${SMOKE_ASINS:-B07JJ5JDXF B0816J4K8Z}"
  # shellcheck disable=SC2086
  if "${PIPELINE[@]}" scrape-amazon $asins >/dev/null 2>&1; then
    n=$(sql "SELECT count(*) FROM store_listings WHERE store_id='amazon';")
    [[ "${n:-0}" -ge 1 ]] && pass "Amazon ingest wrote $n listing(s) through normalize→resolve→embed" \
                          || fail "scrape-amazon ran but wrote 0 amazon listings"
  else
    fail "scrape-amazon failed (check PAAPI_* creds / ASIN validity)"
  fi
else
  skip "no PAAPI_* creds in .env.local — single-store ingest skipped"
fi

# ---------------------------------------------------------------------------
stage "Stage 4 — multi-store ingest (the Phase-2 premise)"
# Cross-store resolution + 8-store comparison require listings from >1 store.
# That needs a runtime path that calls each scraper's crawl_products() and
# persists the result (normalize→resolve→embed→upsert listing). Probe the
# binary for such a subcommand.
help_txt=$("${PIPELINE[@]}" --help 2>&1 || true)
crawl_cmd=""
for cand in crawl full-sync ingest crawl-stores scrape-stores; do
  if grep -qiE "^\s*${cand}\b" <<<"$help_txt"; then crawl_cmd="$cand"; break; fi
done

if [[ -z "$crawl_cmd" ]]; then
  fail "No multi-store ingestion subcommand exists."
  cat <<EOF
${ylw}
  Phase-2 BLOCKER — the validation premise can't be exercised yet.

  The 7 non-Amazon scrapers implement StoreCrawler::crawl_products(), and the
  parser fixtures are green, but NOTHING calls crawl_products() at runtime:
    - 'scrape-amazon' is the only path that creates store_listings (Amazon only)
    - 'price-sync' only REFRESHES prices for listings that already exist
  So no non-Amazon product is ever ingested, no product gets listings from two
  stores, and cross-store entity resolution (ADR-007) never runs on real data.

  To unblock (Session A / pipeline scope): add a one-shot 'full-sync' (or
  'crawl') subcommand that, per store, iterates its categories →
  crawl_products() → normalize → resolve → embed → upsert store_listings
  (mirroring main.rs scrape-amazon, generalized across the crawler registry in
  price_sync::build_crawlers). docs/PHASE2.md PR 6 / SPEC §7.

  Re-run this smoke test once that lands; stages 4–7 will then execute.
${rst}
EOF
else
  pass "found ingestion subcommand: '$crawl_cmd'"
  # full-sync crawls every scrape store's curated category seeds and persists
  # via normalize → resolve → embed → upsert (per ADR-0022 it's one-shot, no
  # daemon). Stores missing creds (proxy-tier without SCRAPE_PROXY_*) or
  # hitting transient errors are logged and skipped; cred-free clean-HTTP
  # stores (CampSaver, Garage Grown Gear) should still produce the cross-
  # store ingest needed for Stage 5.
  if "${PIPELINE[@]}" "$crawl_cmd" >/dev/null 2>&1; then
    pass "$crawl_cmd completed"
  else
    fail "$crawl_cmd exited non-zero — re-run without redirection to see logs"
  fi

  # ----- Stage 5 — cross-store resolution -----
  stage "Stage 5 — cross-store entity resolution"
  multi=$(sql "SELECT count(*) FROM (
                 SELECT product_id FROM store_listings
                 GROUP BY product_id HAVING count(DISTINCT store_id) >= 2
               ) t;")
  cand=$(sql "SELECT count(*) FROM store_listings WHERE match_confidence='CANDIDATE';")
  if [[ "${multi:-0}" -ge 1 ]]; then
    pass "$multi product(s) resolved across ≥2 stores; $cand in CANDIDATE quarantine"
  else
    fail "no product has listings from ≥2 stores — resolution did not merge anything"
    echo "  (CANDIDATE rows: ${cand:-0} — if high, the matcher is quarantining instead of merging)"
  fi

  # ----- Stage 6 — price-sync + Redis comparison hash -----
  stage "Stage 6 — price-sync → Redis 8-store comparison"
  if "${PIPELINE[@]}" price-sync >/dev/null 2>&1; then
    pid=$(sql "SELECT product_id::text FROM store_listings
               GROUP BY product_id HAVING count(DISTINCT store_id) >= 2 LIMIT 1;")
    if [[ -n "$pid" ]]; then
      fields=$("${REDIS[@]}" HLEN "prices:${pid}" 2>/dev/null | tr -d '[:space:]')
      [[ "${fields:-0}" -ge 2 ]] && pass "prices:${pid} holds $fields store quotes" \
                                 || fail "prices:${pid} has ${fields:-0} store field(s), expected ≥2"
    fi
    lu=$("${REDIS[@]}" GET prices:last_updated 2>/dev/null | tr -d '[:space:]')
    [[ -n "$lu" ]] && pass "prices:last_updated stamped ($lu)" \
                   || fail "prices:last_updated not set"
  else
    fail "price-sync run failed"
  fi

  # ----- Stage 7 — API price-comparison endpoint (optional) -----
  stage "Stage 7 — API /prices comparison (optional)"
  if docker compose up -d --build api >/dev/null 2>&1; then
    for _ in $(seq 1 30); do
      curl -fsS http://localhost:8080/actuator/health >/dev/null 2>&1 && break; sleep 2
    done
    # /api/v1/products/{id}/prices — the contract declares {id} as UUID
    # (docs/api/openapi.yaml; @PathVariable UUID id in PricingController), so
    # pass the product UUID, not the slug.
    pid=$(sql "SELECT product_id::text FROM store_listings
               GROUP BY product_id HAVING count(DISTINCT store_id) >= 2 LIMIT 1;")
    if [[ -n "$pid" ]]; then
      body=$(curl -fsS "http://localhost:8080/api/v1/products/${pid}/prices" 2>/dev/null || true)
      cnt=$(grep -o '"storeId"' <<<"$body" | wc -l | tr -d '[:space:]')
      [[ "${cnt:-0}" -ge 2 ]] && pass "GET /products/${pid}/prices returned $cnt store quotes" \
                              || fail "API price comparison returned ${cnt:-0} quotes for ${pid}"
    else
      skip "no cross-store product to query the API with"
    fi
  else
    skip "API container did not start — endpoint check skipped"
  fi
fi

# ---------------------------------------------------------------------------
stage "Summary"
echo "  ${grn}pass:${rst} $pass_count   ${ylw}skip:${rst} $skip_count   ${red}fail:${rst} $fail_count"
[[ "$fail_count" -eq 0 ]] || { echo "${red}Phase-2 smoke test FAILED.${rst}"; exit 1; }
echo "${grn}Phase-2 smoke test passed.${rst}"
