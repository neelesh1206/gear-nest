#!/usr/bin/env bash
#
# Create or update the two Cloud Run Jobs that run the Rust pipeline as
# one-shot binaries (ADR-0022). Idempotent: uses `gcloud run jobs deploy`
# which create-or-updates.
#
#   pipeline-price-sync  → ./gear-nest-pipeline price-sync   (daily)
#   pipeline-full-sync   → ./gear-nest-pipeline full-sync    (weekly)
#
# Both jobs share the pipeline image. Scheduling is wired in a separate
# script (schedule-jobs.sh) so you can rerun this without touching the cron.
#
# Usage:
#   IMAGE_TAG=<sha> scripts/deploy/deploy-jobs.sh
#
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
. "$(dirname "$0")/lib.sh"

require_cmd gcloud
require_active_project

: "${IMAGE_TAG:?set IMAGE_TAG (e.g. export IMAGE_TAG=\$(git rev-parse --short HEAD))}"

IMAGE="$IMAGE_PREFIX/gear-nest-pipeline:$IMAGE_TAG"
log "deploying jobs from $IMAGE"

DATABASE_URL="postgresql:///${CLOUDSQL_DB}?host=/cloudsql/${CLOUDSQL_CONN}&user=${CLOUDSQL_USER}"

deploy_job() {
    local job="$1"; shift
    local args="$1"; shift
    local timeout="$1"; shift

    log "deploy job: $job → $args (timeout: $timeout)"
    gcloud run jobs deploy "$job" \
        --project "$GCP_PROJECT" \
        --region "$GCP_REGION" \
        --image "$IMAGE" \
        --service-account "$PIPELINE_SA_EMAIL" \
        --set-cloudsql-instances "$CLOUDSQL_CONN" \
        --command "/usr/local/bin/gear-nest-pipeline" \
        --args "$args" \
        --cpu 1 \
        --memory 1Gi \
        --task-timeout "$timeout" \
        --max-retries 1 \
        --parallelism 1 \
        --tasks 1 \
        --set-env-vars "DATABASE_URL=${DATABASE_URL}" \
        --set-env-vars "RUST_LOG=info" \
        --set-env-vars "GCP_PROJECT=${GCP_PROJECT}" \
        --set-secrets "REDIS_URL=upstash-redis-url:latest" \
        --set-secrets "HUGGINGFACE_API_KEY=huggingface-api-key:latest" \
        --set-secrets "PAAPI_ACCESS_KEY=paapi-access-key:latest" \
        --set-secrets "PAAPI_SECRET_KEY=paapi-secret-key:latest" \
        --set-secrets "PAAPI_PARTNER_TAG=paapi-partner-tag:latest" \
        --set-secrets "CJ_API_KEY=cj-api-key:latest" \
        --set-secrets "CJ_WEBSITE_ID=cj-website-id:latest" \
        --set-secrets "CJ_REI_ADVERTISER_ID=cj-rei-advertiser-id:latest" \
        --set-secrets "SCRAPE_PROXY_BACKCOUNTRY=scrape-proxy-backcountry:latest" \
        --set-secrets "SCRAPE_PROXY_MOOSEJAW=scrape-proxy-moosejaw:latest" \
        --set-secrets "SCRAPE_PROXY_STEEPANDCHEAP=scrape-proxy-steepandcheap:latest"
}

# price-sync: light, finishes in minutes. 15min ceiling is generous.
deploy_job "$JOB_PRICE_SYNC" "price-sync" "900s"

# full-sync: heavy (8-store crawl + embed). 1h ceiling matches docs/PHASE2.md
# expectations on portfolio-scale catalogs (~50K products).
deploy_job "$JOB_FULL_SYNC"  "full-sync"  "3600s"

log "done. Wire cron with: scripts/deploy/schedule-jobs.sh"
