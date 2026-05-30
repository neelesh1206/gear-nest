#!/usr/bin/env bash
#
# Build + push api / pipeline container images to Artifact Registry.
# Idempotent: re-running rebuilds and overwrites the same SHA tag (Docker
# layer cache makes no-op rebuilds cheap). Images are tagged with the current
# git short-SHA and `latest`; deploy scripts deploy the SHA tag.
#
# Usage:
#   scripts/deploy/build-push-images.sh          # builds both api and pipeline
#   scripts/deploy/build-push-images.sh api      # builds api only
#   scripts/deploy/build-push-images.sh pipeline # builds pipeline only
#
# Env:
#   GCP_PROJECT, GCP_REGION, ARTIFACT_REPO     — see lib.sh
#   IMAGE_PLATFORM=linux/amd64                  — Cloud Run is amd64
#   IMAGE_TAG=<sha>                              — override the SHA tag
#   DEPLOY_YES=1                                 — skip confirm prompts
#
set -euo pipefail
cd "$(dirname "$0")/.."
# shellcheck disable=SC1091
. "$(dirname "$0")/lib.sh"

require_cmd gcloud docker git
require_active_project

WHICH="${1:-all}"
case "$WHICH" in
    all|api|pipeline) ;;
    *) die "unknown target: $WHICH (want: all | api | pipeline)" ;;
esac

if [ -z "${IMAGE_TAG:-}" ]; then
    IMAGE_TAG=$(git -C "$(dirname "$0")/../.." rev-parse --short HEAD 2>/dev/null || echo "manual-$(date +%s)")
fi

log "tagging images with: $IMAGE_TAG (platform: $IMAGE_PLATFORM)"

# One-time per host: tells docker to use gcloud as the Artifact Registry credential helper.
# Re-running is a no-op.
log "configuring docker auth for $IMAGE_HOST"
gcloud auth configure-docker "$IMAGE_HOST" --quiet

build_and_push() {
    local svc="$1"
    local ctx="../$svc"
    local img="$IMAGE_PREFIX/$svc"
    log "building $svc → $img:$IMAGE_TAG"
    docker buildx build \
        --platform "$IMAGE_PLATFORM" \
        --tag "$img:$IMAGE_TAG" \
        --tag "$img:latest" \
        --load \
        "$ctx"
    log "pushing $img:$IMAGE_TAG and :latest"
    docker push "$img:$IMAGE_TAG"
    docker push "$img:latest"
}

case "$WHICH" in
    all)
        build_and_push gear-nest-api
        build_and_push gear-nest-pipeline
        ;;
    api)      build_and_push gear-nest-api ;;
    pipeline) build_and_push gear-nest-pipeline ;;
esac

log "done. Deploy with: IMAGE_TAG=$IMAGE_TAG scripts/deploy/deploy-api.sh"
