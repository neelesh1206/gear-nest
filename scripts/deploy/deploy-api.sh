#!/usr/bin/env bash
#
# Create or update the Cloud Run service for gear-nest-api.
# Idempotent: `gcloud run deploy` updates the service in place if it exists.
#
# Wiring per SPEC §15.4 + ADR-0024:
#   - min-instances=0      (portfolio cost — cold starts accepted)
#   - max-instances=3      (caps blast radius; matches §15.2 service map)
#   - Cloud SQL Auth Proxy via --add-cloudsql-instances (no VPC connector)
#   - secrets pulled from Secret Manager: REDIS_URL, HUGGINGFACE_API_KEY
#   - CORS allow-list points at WEB_ORIGIN (the Vercel project URL)
#
# Usage:
#   IMAGE_TAG=<sha> scripts/deploy/deploy-api.sh
#
# Env:
#   IMAGE_TAG (required)             — image tag from build-push-images.sh
#   WEB_ORIGIN=https://gearnest.app  — set CORS allow-list (see lib.sh)
#
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
. "$(dirname "$0")/lib.sh"

require_cmd gcloud
require_active_project

: "${IMAGE_TAG:?set IMAGE_TAG (e.g. export IMAGE_TAG=\$(git rev-parse --short HEAD))}"

IMAGE="$IMAGE_PREFIX/gear-nest-api:$IMAGE_TAG"
log "deploying $API_SERVICE → $IMAGE"

# SPRING_DATASOURCE_URL uses the Cloud SQL JDBC SocketFactory; cloudSqlInstance
# arg matches the --add-cloudsql-instances connection name. No password —
# the SocketFactory pulls IAM creds from the runtime SA.
JDBC_URL="jdbc:postgresql:///${CLOUDSQL_DB}?cloudSqlInstance=${CLOUDSQL_CONN}&socketFactory=com.google.cloud.sql.postgres.SocketFactory&user=${CLOUDSQL_USER}"

gcloud run deploy "$API_SERVICE" \
    --project "$GCP_PROJECT" \
    --region "$GCP_REGION" \
    --image "$IMAGE" \
    --platform managed \
    --service-account "$API_SA_EMAIL" \
    --add-cloudsql-instances "$CLOUDSQL_CONN" \
    --allow-unauthenticated \
    --port 8080 \
    --cpu 1 \
    --memory 512Mi \
    --min-instances 0 \
    --max-instances 3 \
    --timeout 60s \
    --concurrency 40 \
    --set-env-vars "SPRING_DATASOURCE_URL=${JDBC_URL}" \
    --set-env-vars "SPRING_PROFILES_ACTIVE=prod" \
    --set-env-vars "SESSION_QUESTION_LIMIT=5" \
    --set-env-vars "HUGGINGFACE_EMBEDDING_MODEL=BAAI/bge-small-en-v1.5" \
    --set-env-vars "HUGGINGFACE_LLM_MODEL=Qwen/Qwen2.5-7B-Instruct" \
    --set-env-vars "HUGGINGFACE_BASE_URL=https://router.huggingface.co" \
    --set-env-vars "GCP_PROJECT=${GCP_PROJECT}" \
    --set-env-vars "WEB_ORIGIN=${WEB_ORIGIN}" \
    --set-secrets "REDIS_URL=upstash-redis-url:latest" \
    --set-secrets "HUGGINGFACE_API_KEY=huggingface-api-key:latest"

URL=$(gcloud run services describe "$API_SERVICE" \
    --project "$GCP_PROJECT" --region "$GCP_REGION" \
    --format='value(status.url)')
log "deployed: $URL"
log "next: set NEXT_PUBLIC_API_BASE_URL=$URL on the Vercel project"
