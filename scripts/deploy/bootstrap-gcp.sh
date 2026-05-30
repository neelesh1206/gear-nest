#!/usr/bin/env bash
#
# One-time GCP project bootstrap. Idempotent — every step is either
# `describe || create` or a `set-iam-policy-binding` (server-side dedup).
# Safe to re-run after a partial failure.
#
# What it does:
#   1. Enable required APIs
#   2. Create the Artifact Registry repo
#   3. Create the two service accounts (api, pipeline)
#   4. Grant Cloud SQL Client + Cloud Run Invoker to the right SAs
#   5. Create the Cloud SQL Postgres instance (db-f1-micro, 10GB SSD)
#      with pgvector enabled via the cloudsql.enable_pgvector flag
#      (per ADR-0024: public IP + Cloud SQL Auth Proxy, no VPC connector)
#   6. Create the application database and user
#
# What it does NOT do (do these manually before running):
#   - Create the project + link billing  (`gcloud projects create`,
#     `gcloud billing projects link`)  — these cost money / can't undo
#   - Set the GCP $30/mo budget alert    (SPEC §15.5)
#   - Provision Upstash Redis             (browser, free tier)
#
# Usage:
#   GCP_PROJECT=gearnest-prod scripts/deploy/bootstrap-gcp.sh
#
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
. "$(dirname "$0")/lib.sh"

require_cmd gcloud
require_active_project

# 1. APIs ---------------------------------------------------------------------
log "enabling required APIs (idempotent)"
gcloud services enable \
    run.googleapis.com \
    sqladmin.googleapis.com \
    artifactregistry.googleapis.com \
    cloudscheduler.googleapis.com \
    secretmanager.googleapis.com \
    iam.googleapis.com \
    billingbudgets.googleapis.com \
    --project "$GCP_PROJECT"

# 2. Artifact Registry --------------------------------------------------------
if gcloud artifacts repositories describe "$ARTIFACT_REPO" \
    --location "$GCP_REGION" --project "$GCP_PROJECT" >/dev/null 2>&1; then
    log "artifact registry $ARTIFACT_REPO/$GCP_REGION exists"
else
    log "creating artifact registry $ARTIFACT_REPO/$GCP_REGION"
    gcloud artifacts repositories create "$ARTIFACT_REPO" \
        --repository-format=docker \
        --location="$GCP_REGION" \
        --description="GearNest container images" \
        --project "$GCP_PROJECT"
fi

# 3. Service accounts ---------------------------------------------------------
ensure_sa() {
    local name="$1" display="$2"
    local email="${name}@${GCP_PROJECT}.iam.gserviceaccount.com"
    if gcloud iam service-accounts describe "$email" --project "$GCP_PROJECT" >/dev/null 2>&1; then
        log "service account $name exists"
    else
        log "creating service account $name"
        gcloud iam service-accounts create "$name" \
            --display-name "$display" \
            --project "$GCP_PROJECT"
    fi
}
ensure_sa "$API_SA"      "GearNest API"
ensure_sa "$PIPELINE_SA" "GearNest Pipeline"

# 4. IAM ----------------------------------------------------------------------
grant() {
    local member="$1" role="$2"
    log "iam: $member → $role"
    gcloud projects add-iam-policy-binding "$GCP_PROJECT" \
        --member "$member" --role "$role" --condition=None >/dev/null
}

for sa in "$API_SA_EMAIL" "$PIPELINE_SA_EMAIL"; do
    grant "serviceAccount:$sa" "roles/cloudsql.client"
    grant "serviceAccount:$sa" "roles/secretmanager.secretAccessor"
    grant "serviceAccount:$sa" "roles/artifactregistry.reader"
done

# Pipeline SA also acts as the Cloud Scheduler caller (schedule-jobs.sh)
# and as the runtime SA for the Jobs themselves; it needs run.invoker on
# the Cloud Run Jobs to be able to call jobs.run.
grant "serviceAccount:$PIPELINE_SA_EMAIL" "roles/run.invoker"

# 5. Cloud SQL ----------------------------------------------------------------
# db-f1-micro (~$10/mo per ADR-0024). pgvector flag is on by default in
# Cloud SQL Postgres 16+; cloudsql.enable_pgvector=on is set for safety
# (no-op if already on).
if gcloud sql instances describe "$CLOUDSQL_INSTANCE" --project "$GCP_PROJECT" >/dev/null 2>&1; then
    log "cloud sql instance $CLOUDSQL_INSTANCE exists"
else
    log "creating cloud sql instance $CLOUDSQL_INSTANCE (db-f1-micro, 10GB SSD)"
    log "  this takes ~5 minutes the first time"
    confirm "create Cloud SQL instance (incurs ~\$10/mo)?"
    gcloud sql instances create "$CLOUDSQL_INSTANCE" \
        --project "$GCP_PROJECT" \
        --region "$GCP_REGION" \
        --database-version "POSTGRES_16" \
        --tier "db-f1-micro" \
        --storage-type "SSD" \
        --storage-size "10GB" \
        --storage-auto-increase \
        --backup \
        --backup-start-time "07:00" \
        --retained-backups-count 7 \
        --database-flags "cloudsql.enable_pgvector=on" \
        --availability-type "zonal"
fi

# Cloud SQL IAM authentication — Cloud Run / Jobs use Workload Identity, no
# password. This flag is harmless if already on.
gcloud sql instances patch "$CLOUDSQL_INSTANCE" \
    --project "$GCP_PROJECT" \
    --database-flags "cloudsql.enable_pgvector=on,cloudsql.iam_authentication=on" \
    --quiet || warn "patch (re-)set of database flags failed; re-run if Cloud SQL was mid-operation"

# Application database
if gcloud sql databases describe "$CLOUDSQL_DB" \
    --instance "$CLOUDSQL_INSTANCE" --project "$GCP_PROJECT" >/dev/null 2>&1; then
    log "database $CLOUDSQL_DB exists"
else
    log "creating database $CLOUDSQL_DB"
    gcloud sql databases create "$CLOUDSQL_DB" \
        --instance "$CLOUDSQL_INSTANCE" --project "$GCP_PROJECT"
fi

# IAM-authenticated DB users for each SA. The username for an IAM service
# account user is the SA email (Cloud SQL truncates to first 63 chars).
for sa in "$API_SA_EMAIL" "$PIPELINE_SA_EMAIL"; do
    if gcloud sql users list --instance "$CLOUDSQL_INSTANCE" --project "$GCP_PROJECT" \
        --format="value(name)" | grep -qx "$sa"; then
        log "cloud sql IAM user exists: $sa"
    else
        log "creating cloud sql IAM user: $sa"
        gcloud sql users create "$sa" \
            --instance "$CLOUDSQL_INSTANCE" \
            --project "$GCP_PROJECT" \
            --type "CLOUD_IAM_SERVICE_ACCOUNT"
    fi
done

log "bootstrap done."
log "next steps:"
log "  scripts/deploy/sync-secrets.sh                 # populate Secret Manager from .env.production"
log "  scripts/deploy/build-push-images.sh            # build + push api / pipeline images"
log "  IMAGE_TAG=\$(git rev-parse --short HEAD) scripts/deploy/deploy-api.sh"
log "  IMAGE_TAG=\$(git rev-parse --short HEAD) scripts/deploy/deploy-jobs.sh"
log "  scripts/deploy/schedule-jobs.sh"
log ""
log "Once Cloud SQL is reachable, apply migrations from your laptop with the Cloud SQL Auth Proxy:"
log "  cloud-sql-proxy ${CLOUDSQL_CONN} &"
log "  DATABASE_URL=postgres://${CLOUDSQL_USER}@127.0.0.1:5432/${CLOUDSQL_DB} \\\\"
log "      (cd gear-nest-pipeline && cargo run -- migrate)"
