#!/usr/bin/env bash
#
# Mirror a local .env.production file into GCP Secret Manager. Idempotent —
# secrets that exist get a new version added; secrets that don't exist are
# created. Empty / missing values are skipped (the Cloud Run --set-secrets
# wiring will simply fail at deploy if a required secret is absent).
#
# .env.production lives ONLY on the operator's machine. It is gitignored and
# never committed. The single source of truth for production secrets is
# Secret Manager itself — this script just bridges from the file you keep
# in 1Password / a sealed vault to GCP.
#
# Usage:
#   scripts/deploy/sync-secrets.sh                   # reads .env.production
#   scripts/deploy/sync-secrets.sh path/to/env       # reads named file
#
# The mapping (env var name → secret name) is fixed by deploy-api.sh and
# deploy-jobs.sh; do not rename either side without updating both.
#
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
. "$(dirname "$0")/lib.sh"

require_cmd gcloud
require_active_project

ENV_FILE="${1:-../.env.production}"
[ -f "$ENV_FILE" ] || die "env file not found: $ENV_FILE (copy .env.example, fill in production values, save outside the repo)"

# env-var → secret-name. Underscores → dashes, lowercase. Anything not in this
# list is ignored even if present in the env file (forces explicit allow-list).
declare -A MAP=(
    [REDIS_URL]=upstash-redis-url
    [HUGGINGFACE_API_KEY]=huggingface-api-key
    [PAAPI_ACCESS_KEY]=paapi-access-key
    [PAAPI_SECRET_KEY]=paapi-secret-key
    [PAAPI_PARTNER_TAG]=paapi-partner-tag
    [CJ_API_KEY]=cj-api-key
    [CJ_WEBSITE_ID]=cj-website-id
    [CJ_REI_ADVERTISER_ID]=cj-rei-advertiser-id
    [SCRAPE_PROXY_BACKCOUNTRY]=scrape-proxy-backcountry
    [SCRAPE_PROXY_MOOSEJAW]=scrape-proxy-moosejaw
    [SCRAPE_PROXY_STEEPANDCHEAP]=scrape-proxy-steepandcheap
)

# Service accounts that need read access on every secret. Granted on
# create; re-granted via add-iam-policy-binding (idempotent on identical bindings).
ACCESSORS=("$API_SA_EMAIL" "$PIPELINE_SA_EMAIL")

upsert_secret() {
    local name="$1"
    local value="$2"

    if gcloud secrets describe "$name" --project "$GCP_PROJECT" >/dev/null 2>&1; then
        log "secret $name exists — adding new version"
        printf '%s' "$value" | gcloud secrets versions add "$name" \
            --project "$GCP_PROJECT" --data-file=-
    else
        log "secret $name does not exist — creating"
        printf '%s' "$value" | gcloud secrets create "$name" \
            --project "$GCP_PROJECT" \
            --replication-policy="automatic" \
            --data-file=-
    fi

    for sa in "${ACCESSORS[@]}"; do
        gcloud secrets add-iam-policy-binding "$name" \
            --project "$GCP_PROJECT" \
            --member "serviceAccount:$sa" \
            --role "roles/secretmanager.secretAccessor" \
            --condition=None >/dev/null
    done
}

log "syncing secrets from $ENV_FILE"
synced=0; skipped=0
while IFS= read -r line || [ -n "$line" ]; do
    # strip comments + blank lines
    case "$line" in
        ''|\#*) continue ;;
    esac
    # split on first '='
    key="${line%%=*}"
    val="${line#*=}"
    # strip surrounding quotes
    val="${val%\"}"; val="${val#\"}"
    val="${val%\'}"; val="${val#\'}"

    secret="${MAP[$key]:-}"
    if [ -z "$secret" ]; then
        continue
    fi
    if [ -z "$val" ]; then
        warn "skipping $key (empty value)"
        skipped=$((skipped+1))
        continue
    fi
    upsert_secret "$secret" "$val"
    synced=$((synced+1))
done < "$ENV_FILE"

log "synced $synced secret(s), skipped $skipped empty value(s)"
