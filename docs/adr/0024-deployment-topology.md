# ADR-0024: Deployment topology — Cloud SQL tier, IP exposure, image build, region

**Status:** Accepted
**Date:** 2026-05-30
**Owner:** Session 0 (contract track)

## Context
ADR-012 pinned the high-level stack (Cloud Run + Cloud SQL + Upstash Redis,
$30/mo hard cap) and ADR-0022 pinned the pipeline as one-shot Cloud Run Jobs
triggered by Cloud Scheduler. Those two left four mechanical choices open that
the deploy playbook (`docs/DEPLOY.md`) needs to commit to before any
`gcloud` command runs:

1. Cloud SQL machine tier
2. Cloud SQL IP exposure (public + Authorized Networks vs. private VPC)
3. Container image build (Cloud Build vs. local `docker push`)
4. Region count (single vs. multi-region)

## Decision

### 1. Cloud SQL: `db-f1-micro`, 10 GB SSD, daily backups, point-in-time recovery off
Shared-core, ~$10/mo. Sized for the §5 "Target Scale" budget
(~50K products, ~3M review chunks, ~30K price-history rows/day after monthly
partition prune). PITR adds ~$3/mo for WAL retention and isn't worth it for
a portfolio project — daily auto-backup with 7-day retention is enough.
Upgrade path is one `gcloud sql instances patch --tier=db-g1-small` when the
review chunk count crosses ~10M.

### 2. Cloud SQL: **public IP + Authorized Networks**, not private VPC
SPEC §15.2 sketched a private IP via a Serverless VPC Access connector. That
connector costs **$10–14/mo** at the smallest size (`e2-micro`, 2 instances),
nearly doubling the infra bill for a setup whose only client is one Cloud Run
service and a few one-shot Jobs in the same project.

Instead:
- Cloud SQL instance gets a public IP.
- Connection from Cloud Run / Cloud Run Jobs goes through the **Cloud SQL
  Auth Proxy** (`--add-cloudsql-instances`) — TLS + IAM-authenticated, no
  password over the wire.
- Authorized Networks list stays **empty** (no plain-IP allowlist; only the
  Cloud SQL Auth Proxy path is open).

This keeps the security posture (no public password auth, no plain-IP path)
while saving the connector cost. Revisit if a second client outside Cloud Run
ever needs the DB.

### 3. Image build: **local `docker push` → Artifact Registry**, not Cloud Build
Cloud Build is free up to 120 build-minutes/day, but it adds a moving part
(triggers, build configs, separate service account, build-cache buckets) for
no win on a single-developer project. Local `docker buildx build --platform
linux/amd64` + `gcloud auth configure-docker` + `docker push` is one fewer
GCP product to wire up and keeps the image build deterministic against the
developer's checked-out tree. CI parity is identical — GitHub Actions does
the same `docker push` flow in `.github/workflows/deploy-api.yml` (SPEC §15.6).

Trade-off: building locally on Apple Silicon requires `--platform linux/amd64`
since Cloud Run is amd64. `scripts/deploy/build-push-images.sh` sets that
flag explicitly.

### 4. Region: **single region (`us-central1`)**
Multi-region buys nothing for a portfolio project — latency budget is dominated
by HuggingFace Inference Provider calls (200–800 ms p50), not by Cloud Run RTT.
Multi-region Cloud SQL alone runs ~$50/mo, busting the $30 cap. Co-locate
everything in `us-central1`:
- Cloud Run (api), Cloud Run Jobs (price-sync, full-sync)
- Cloud SQL (Postgres + pgvector)
- Artifact Registry repo
- Cloud Scheduler triggers

Vercel + Upstash are external and global; their CDN/edge handles geography.

## Trade-offs accepted
- **No DR / failover.** Single-region Cloud SQL on `db-f1-micro` has ~99.95%
  SLA. A regional outage means the API is down — acceptable for a portfolio
  project; data is preserved by daily backup.
- **No PITR.** Worst-case data loss on Cloud SQL restore is 24h. Mitigated by
  the fact that `price_history` and `reviews` are re-derivable from a pipeline
  run, and `store_listings` / `products` change slowly.
- **Public IP on the DB** is a posture downgrade from SPEC §15.2's private-VPC
  sketch, but the Cloud SQL Auth Proxy makes the actual exposure equivalent
  (no password-over-internet, IAM-gated). Documented above so a future
  reviewer doesn't read "public IP" and assume sloppy security.
- **No CI-driven image builds today.** GitHub Actions workflow under SPEC §15.6
  exists, but day-one deploy is local push from the developer's machine to
  prove the path end-to-end; the Actions workflow becomes the default once a
  Workload Identity Federation pool is set up (not in scope for this PR).

## Why not the alternatives
- `db-g1-small` (~$25/mo): 2.5× the price for headroom Phase 1 doesn't need.
- `db-custom-1-3840` (~$30/mo): same — and it alone hits the cap.
- Memorystore Redis ($35/mo minimum): excluded already by ADR-012; reaffirmed.
- Cloud Build + triggers: real value when a team needs hands-off pushes to
  `main`. Not the situation here.
- Multi-region active-active: overkill; busts the budget by ~3×.
