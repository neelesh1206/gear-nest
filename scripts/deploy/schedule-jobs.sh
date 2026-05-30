#!/usr/bin/env bash
#
# Wire Cloud Scheduler triggers for the two Cloud Run Jobs (ADR-0022).
# Idempotent: deletes any existing schedule with the same name first
# (`gcloud scheduler jobs create` errors if the name exists; there's no
# `deploy` verb for scheduler).
#
# Crons (UTC):
#   price-sync-daily   `0 6 * * *`    daily 06:00 UTC
#   full-sync-weekly   `0 2 * * 0`    Sunday 02:00 UTC
#
# Both fire well outside US-peak browsing (US-Pacific 22:00 / 19:00 prior),
# so a slow sync doesn't compete with API latency. UTC chosen — the schedule
# lives next to GCP infra and shouldn't drift across daylight-saving.
#
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
. "$(dirname "$0")/lib.sh"

require_cmd gcloud
require_active_project

# Cloud Scheduler invokes Jobs via the Cloud Run Admin API. The SA needs
# roles/run.invoker on the target Job; the trigger script doesn't grant it,
# bootstrap-gcp.sh does (one-time IAM step).
SCHEDULER_SA="${SCHEDULER_SA:-$PIPELINE_SA_EMAIL}"
URI_PREFIX="https://${GCP_REGION}-run.googleapis.com/apis/run.googleapis.com/v1"

upsert_cron() {
    local name="$1"
    local cron="$2"
    local job="$3"

    if gcloud scheduler jobs describe "$name" \
        --project "$GCP_PROJECT" --location "$GCP_REGION" >/dev/null 2>&1; then
        log "scheduler job $name exists — updating"
        gcloud scheduler jobs update http "$name" \
            --project "$GCP_PROJECT" \
            --location "$GCP_REGION" \
            --schedule "$cron" \
            --time-zone "UTC" \
            --uri "${URI_PREFIX}/namespaces/${GCP_PROJECT}/jobs/${job}:run" \
            --http-method POST \
            --oauth-service-account-email "$SCHEDULER_SA" \
            --attempt-deadline "300s"
    else
        log "scheduler job $name does not exist — creating"
        gcloud scheduler jobs create http "$name" \
            --project "$GCP_PROJECT" \
            --location "$GCP_REGION" \
            --schedule "$cron" \
            --time-zone "UTC" \
            --uri "${URI_PREFIX}/namespaces/${GCP_PROJECT}/jobs/${job}:run" \
            --http-method POST \
            --oauth-service-account-email "$SCHEDULER_SA" \
            --attempt-deadline "300s" \
            --description "Cloud Scheduler trigger for $job (ADR-0022)"
    fi
}

upsert_cron "$SCHED_PRICE_SYNC" "0 6 * * *" "$JOB_PRICE_SYNC"
upsert_cron "$SCHED_FULL_SYNC"  "0 2 * * 0" "$JOB_FULL_SYNC"

log "done. Triggers:"
log "  $SCHED_PRICE_SYNC → 0 6 * * * UTC → $JOB_PRICE_SYNC"
log "  $SCHED_FULL_SYNC  → 0 2 * * 0 UTC → $JOB_FULL_SYNC"
log "force-run for smoke test: gcloud scheduler jobs run $SCHED_PRICE_SYNC --location $GCP_REGION"
