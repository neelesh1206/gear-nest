# Shared config + helpers for scripts/deploy/*.sh.
# Source-only — has no shebang and does nothing on its own.
#
# All scripts read deploy config from env, with portfolio defaults baked in.
# Override per-invocation:  GCP_PROJECT=gearnest-prod scripts/deploy/foo.sh

: "${GCP_PROJECT:=gearnest-prod}"
: "${GCP_REGION:=us-central1}"
: "${ARTIFACT_REPO:=gearnest}"
: "${CLOUDSQL_INSTANCE:=gearnest-db}"
: "${CLOUDSQL_DB:=gearnest}"
: "${CLOUDSQL_USER:=gearnest}"
: "${API_SERVICE:=gearnest-api}"
: "${API_SA:=gearnest-api}"
: "${PIPELINE_SA:=gearnest-pipeline}"
: "${JOB_PRICE_SYNC:=pipeline-price-sync}"
: "${JOB_FULL_SYNC:=pipeline-full-sync}"
: "${SCHED_PRICE_SYNC:=price-sync-daily}"
: "${SCHED_FULL_SYNC:=full-sync-weekly}"
# Vercel deploys gear-nest-web; CORS allow-list for the API. Override for previews.
: "${WEB_ORIGIN:=https://gearnest.app}"
# Override only for amd64 hosts; default chosen so Apple Silicon devs don't
# accidentally push arm64 images Cloud Run can't run.
: "${IMAGE_PLATFORM:=linux/amd64}"

export GCP_PROJECT GCP_REGION ARTIFACT_REPO CLOUDSQL_INSTANCE CLOUDSQL_DB CLOUDSQL_USER
export API_SERVICE API_SA PIPELINE_SA JOB_PRICE_SYNC JOB_FULL_SYNC
export SCHED_PRICE_SYNC SCHED_FULL_SYNC WEB_ORIGIN IMAGE_PLATFORM

IMAGE_HOST="${GCP_REGION}-docker.pkg.dev"
IMAGE_PREFIX="${IMAGE_HOST}/${GCP_PROJECT}/${ARTIFACT_REPO}"
API_SA_EMAIL="${API_SA}@${GCP_PROJECT}.iam.gserviceaccount.com"
PIPELINE_SA_EMAIL="${PIPELINE_SA}@${GCP_PROJECT}.iam.gserviceaccount.com"
CLOUDSQL_CONN="${GCP_PROJECT}:${GCP_REGION}:${CLOUDSQL_INSTANCE}"

export IMAGE_HOST IMAGE_PREFIX API_SA_EMAIL PIPELINE_SA_EMAIL CLOUDSQL_CONN

log()  { printf '\033[1m▶ %s\033[0m\n' "$*" >&2; }
warn() { printf '\033[33m⚠ %s\033[0m\n' "$*" >&2; }
die()  { printf '\033[31m✖ %s\033[0m\n' "$*" >&2; exit 1; }

require_cmd() {
    local missing=0
    for c in "$@"; do
        if ! command -v "$c" >/dev/null 2>&1; then
            warn "missing required command: $c"
            missing=1
        fi
    done
    [ "$missing" -eq 0 ] || die "install the commands above and retry"
}

require_active_project() {
    local active
    active=$(gcloud config get-value project 2>/dev/null || true)
    if [ "$active" != "$GCP_PROJECT" ]; then
        die "gcloud project is '$active'; expected '$GCP_PROJECT'. Run: gcloud config set project $GCP_PROJECT"
    fi
}

confirm() {
    local prompt="${1:-Continue?} [y/N] "
    if [ "${DEPLOY_YES:-0}" = "1" ]; then
        return 0
    fi
    local reply
    read -r -p "$prompt" reply
    case "$reply" in
        y|Y|yes|YES) return 0 ;;
        *) die "aborted" ;;
    esac
}
