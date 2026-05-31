#!/usr/bin/env bash
#
# Local demo: brings up Postgres + Redis + API in Docker, seeds 5 demo
# products with cross-store listings + live Redis prices, then runs the
# web UI on the host (`npm run dev`) so the browser can reach the API at
# http://localhost:8080.
#
# Why web on host (not docker compose --profile web):
#   The compose web build bakes NEXT_PUBLIC_API_BASE into the bundle and
#   the value `http://api:8080` only resolves inside the Docker network.
#   Browser-side fetches need `http://localhost:8080`, which the dev
#   server picks up via the default in gear-nest-web/lib/api/client.ts.
#
# Usage:
#   scripts/local_demo.sh                # start
#   scripts/local_demo.sh down           # tear it all down (drops the demo db)
#
set -uo pipefail
cd "$(dirname "$0")/.."

bold=$(printf '\033[1m'); grn=$(printf '\033[32m'); red=$(printf '\033[31m'); rst=$(printf '\033[0m')

if [[ "${1:-}" == "down" ]]; then
  echo "Tearing down docker stack + killing host web dev server…"
  docker compose --profile api down -v
  pkill -f "next dev" 2>/dev/null || true
  rm -f /tmp/gearnest-web.log /tmp/gearnest-web.pid
  echo "${grn}done.${rst}"
  exit 0
fi

if ! docker info >/dev/null 2>&1; then
  echo "Docker daemon not reachable. Start Colima: ${bold}colima start${rst}"
  exit 1
fi

# Order matters:
#   1) pg + redis first (preserves volume between runs).
#   2) Seed while waiting for pg health.
#   3) API LAST, with --force-recreate so it always gets a fresh port binding.
#      Without --force-recreate, a container left over from a failed prior run
#      (e.g. port-8080 conflict at the time) gets reused with NO port map,
#      and the API is unreachable from the host despite "running".
#   4) Real HTTP probe of the API — fail loudly if it isn't reachable, instead
#      of declaring "Ready" while the UI is silently broken.

echo "${bold}1/6${rst} bringing up postgres + redis…"
docker compose up -d postgres redis >/dev/null

echo "${bold}2/6${rst} waiting for postgres health…"
for _ in $(seq 1 30); do
  [[ "$(docker compose ps --format '{{.Health}}' postgres)" == "healthy" ]] && break
  sleep 2
done
if [[ "$(docker compose ps --format '{{.Health}}' postgres)" != "healthy" ]]; then
  echo "${red}postgres did not become healthy${rst}"; exit 1
fi

echo "${bold}3/6${rst} seeding products + listings…"
docker compose exec -T postgres psql -U gearnest -d gearnest -q < scripts/seed/local_demo.sql

echo "${bold}4/6${rst} seeding Redis prices (fresh fetched_at so the API treats them as live)…"
NOW=$(date -u +"%Y-%m-%dT%H:%M:%SZ")
seed_price () {
  local product_id="$1" store="$2" listing_id="$3" price="$4" in_stock="$5"
  local payload
  payload=$(printf '{"listing_id":"%s","price":"%s","in_stock":%s,"fetched_at":"%s","jitter_secs":0}' \
              "$listing_id" "$price" "$in_stock" "$NOW")
  docker compose exec -T redis redis-cli HSET "prices:${product_id}" "$store" "$payload" >/dev/null
}

# MSR PocketRocket 2
seed_price 11111111-0000-0000-0000-000000000001 amazon       22222222-0001-0001-0000-000000000000 49.95  true
seed_price 11111111-0000-0000-0000-000000000001 rei          22222222-0001-0002-0000-000000000000 49.95  true
seed_price 11111111-0000-0000-0000-000000000001 campsaver    22222222-0001-0003-0000-000000000000 44.99  true

# Garmin Fenix 7
seed_price 11111111-0000-0000-0000-000000000002 amazon       22222222-0002-0001-0000-000000000000 549.99 true
seed_price 11111111-0000-0000-0000-000000000002 rei          22222222-0002-0002-0000-000000000000 599.95 true
seed_price 11111111-0000-0000-0000-000000000002 backcountry  22222222-0002-0004-0000-000000000000 579.00 false

# Patagonia Down Sweater
seed_price 11111111-0000-0000-0000-000000000003 rei          22222222-0003-0002-0000-000000000000 279.00 true
seed_price 11111111-0000-0000-0000-000000000003 backcountry  22222222-0003-0004-0000-000000000000 251.10 true
seed_price 11111111-0000-0000-0000-000000000003 moosejaw     22222222-0003-0005-0000-000000000000 269.00 true

# Black Diamond Spot 400
seed_price 11111111-0000-0000-0000-000000000004 amazon            22222222-0004-0001-0000-000000000000 49.95 true
seed_price 11111111-0000-0000-0000-000000000004 rei               22222222-0004-0002-0000-000000000000 54.95 true
seed_price 11111111-0000-0000-0000-000000000004 campsaver         22222222-0004-0003-0000-000000000000 47.99 true
seed_price 11111111-0000-0000-0000-000000000004 garagerowngear    22222222-0004-0008-0000-000000000000 49.99 true

# Osprey Atmos AG 65
seed_price 11111111-0000-0000-0000-000000000005 amazon       22222222-0005-0001-0000-000000000000 339.95 true
seed_price 11111111-0000-0000-0000-000000000005 rei          22222222-0005-0002-0000-000000000000 349.95 true
seed_price 11111111-0000-0000-0000-000000000005 backcountry  22222222-0005-0004-0000-000000000000 314.96 true
seed_price 11111111-0000-0000-0000-000000000005 moosejaw     22222222-0005-0005-0000-000000000000 329.95 false

docker compose exec -T redis redis-cli SET "prices:last_updated" "$NOW" >/dev/null

echo "${bold}5/6${rst} bringing up API (force-recreate so the port binding is fresh)…"
if ! docker compose --profile api up -d --build --force-recreate api >/dev/null 2>&1; then
  echo "${red}API container failed to start. Common cause: port 8080 already bound by another process.${rst}"
  echo "  Check with:  lsof -nP -iTCP:8080 -sTCP:LISTEN"
  echo "  Or another Docker compose project: docker ps --format '{{.Names}}\\t{{.Ports}}' | grep 8080"
  exit 1
fi

# Real HTTP probe — Spring Boot startup is ~20s after the container reports
# "Started", so the only honest readiness check is hitting an actual endpoint.
ready=0
for i in $(seq 1 40); do
  code=$(curl -s -o /dev/null -w '%{http_code}' 'http://localhost:8080/api/v1/products/search?q=msr' 2>/dev/null)
  if [[ "$code" =~ ^[23] ]]; then ready=1; break; fi
  sleep 3
done
if [[ "$ready" -ne 1 ]]; then
  echo "${red}API not reachable on http://localhost:8080 after 120s.${rst}"
  echo "  Inspect:  docker logs gear-nest-api-1"
  exit 1
fi

echo "${bold}6/6${rst} starting web dev server on http://localhost:3000 (background)…"
( cd gear-nest-web && nohup npm run dev > /tmp/gearnest-web.log 2>&1 & echo $! > /tmp/gearnest-web.pid )

for _ in $(seq 1 30); do
  curl -fsS http://localhost:3000 >/dev/null 2>&1 && break
  sleep 2
done

echo
echo "${grn}${bold}Ready.${rst}"
echo "  Web:  ${bold}http://localhost:3000${rst}"
echo "  API:  ${bold}http://localhost:8080${rst}    (e.g. /api/v1/products/search?q=msr)"
echo
echo "Demo products to try:"
echo "  - ${bold}/products/msr-pocketrocket-2${rst}      (3 stores)"
echo "  - ${bold}/products/garmin-fenix-7${rst}           (3 stores)"
echo "  - ${bold}/products/osprey-atmos-ag-65${rst}       (4 stores)"
echo "  - ${bold}/products/black-diamond-spot-400${rst}   (4 stores)"
echo "  - ${bold}/products/patagonia-down-sweater${rst}   (3 stores)"
echo
echo "Tail web logs: ${bold}tail -f /tmp/gearnest-web.log${rst}"
echo "Tear down:     ${bold}scripts/local_demo.sh down${rst}"
