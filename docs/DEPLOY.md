# GearNest — Deployment Playbook (GCP + Vercel + Upstash)

This is the executable guide to go from "nothing exists in GCP" to a running
production stack. It follows the topology pinned by SPEC §15.2, [ADR-012][adr12]
(GCP over Fly.io, $30/mo cap), [ADR-0022][adr22] (external scheduling), and
[ADR-0024][adr24] (deploy topology).

[adr12]: ./adr/0012-gcp-over-flyio.md
[adr22]: ./adr/0022-external-scheduling-over-in-process-cron.md
[adr24]: ./adr/0024-deployment-topology.md

> **What you do** vs. **what scripts do:** anything that costs money (creating
> the GCP project, linking billing, setting the budget, provisioning Upstash)
> is **manual** in this guide — there's no script that can undo a billing
> mistake. Everything else (APIs, IAM, Cloud SQL, Cloud Run, Cloud Scheduler,
> Secret Manager) is idempotent and lives under `scripts/deploy/`.

---

## 0. Prerequisites (one-time, on the operator's machine)

- `gcloud` CLI ≥ 481 (`gcloud version`)
- `docker` with `buildx` (for `--platform linux/amd64`); Apple Silicon needs this
- `git`
- A GCP **billing account** ID you control
- (Later) A Vercel account with the `gear-nest-web` project already imported

```bash
gcloud auth login
gcloud auth application-default login   # for any Terraform-like tooling later
```

---

## 1. Manual: create the GCP project + link billing + set the $30/mo cap

These steps **cost money or commit billing**; the scripts deliberately do not
do them.

```bash
# 1a. Create the project (project ID must be globally unique)
gcloud projects create gearnest-prod --name="GearNest"
gcloud config set project gearnest-prod

# 1b. Find your billing account ID
gcloud billing accounts list
# → 0X0X0X-0X0X0X-0X0X0X

# 1c. Link billing
gcloud billing projects link gearnest-prod \
    --billing-account=YOUR_BILLING_ACCOUNT_ID

# 1d. Set the $30/mo hard cap (SPEC §15.5). This is the most important step.
#     Do this BEFORE running any other deploy script — if anything later
#     misconfigures and starts burning money, the cap is your stop button.
gcloud billing budgets create \
    --billing-account=YOUR_BILLING_ACCOUNT_ID \
    --display-name="GearNest \$30 Hard Cap" \
    --budget-amount=30USD \
    --threshold-rule=percent=50 \
    --threshold-rule=percent=90 \
    --threshold-rule=percent=100
```

The optional "auto-disable billing at 100%" Cloud Function from SPEC §15.5
is the recommended safety net but is **not** part of this PR — wire it after
the stack is up and you've watched a real bill cycle.

---

## 2. Provision Upstash Redis (manual, free tier)

GCP Memorystore is excluded by ADR-012 (its ~$35/mo floor alone busts the cap).
Use Upstash:

1. https://console.upstash.com/ → **Create Database**
2. Type: **Regional**, Region: **us-east-1** (closest free-tier region to
   `us-central1`; lat ≈ 30 ms — acceptable)
3. TLS: **enabled**
4. Eviction: **noeviction** (the SWR price cache is sized so we never need
   eviction; loud failure is preferable to silent key loss)
5. Copy the `redis://` connection URL — you'll put it in
   `.env.production` below

Free tier is 10K commands/day. Production usage at portfolio scale is
~2–3K commands/day (8-store price-sync + chat sessions), so we sit well
under the ceiling.

---

## 3. Author the production env file (local-only, never committed)

Copy the template, fill in production values, **keep the file outside the
repo or in 1Password** — `.env.production` is gitignored but it's still a
plaintext file with live credentials.

```bash
cp .env.example ../.env.production
$EDITOR ../.env.production
```

Required for a complete deploy:

| Variable                       | Source                                            |
|--------------------------------|---------------------------------------------------|
| `REDIS_URL`                    | Upstash console (§2)                              |
| `HUGGINGFACE_API_KEY`          | https://huggingface.co/settings/tokens (read role) |
| `PAAPI_ACCESS_KEY` / `_SECRET_KEY` / `_PARTNER_TAG` | Amazon Associates → PA-API |
| `CJ_API_KEY` / `_WEBSITE_ID` / `_REI_ADVERTISER_ID` | Commission Junction account |
| `SCRAPE_PROXY_BACKCOUNTRY`     | Bright Data / Smartproxy residential endpoint     |
| `SCRAPE_PROXY_MOOSEJAW`        | (same)                                            |
| `SCRAPE_PROXY_STEEPANDCHEAP`   | (same)                                            |

> Anything not in `sync-secrets.sh`'s allow-list (e.g. `PAAPI_HOST`,
> `HUGGINGFACE_BASE_URL`) goes in as a plain Cloud Run env var, not a secret.

---

## 4. Bootstrap GCP (idempotent — script)

Runs everything that's safe to automate: API enables, Artifact Registry,
service accounts, IAM bindings, the Cloud SQL Postgres 16+pgvector instance
(public IP + Cloud SQL Auth Proxy per [ADR-0024][adr24]), the application
database, and IAM-authenticated DB users.

```bash
# Default config (project: gearnest-prod, region: us-central1). Override via env.
scripts/deploy/bootstrap-gcp.sh
```

The script prompts before creating the Cloud SQL instance (the only step
inside it that incurs ongoing cost). To skip the prompt for re-runs:
`DEPLOY_YES=1 scripts/deploy/bootstrap-gcp.sh`.

Per [ADR-0024][adr24]: this provisions a **public-IP** Cloud SQL instance.
Access from Cloud Run is via the Cloud SQL Auth Proxy (TLS, IAM-authenticated),
**not** plain IP — Authorized Networks stays empty. This skips the $10–14/mo
Serverless VPC Access connector that a private-IP setup would require.

---

## 5. Push secrets into Secret Manager (idempotent — script)

```bash
scripts/deploy/sync-secrets.sh
# default: reads ../.env.production
# or pass a path explicitly:
scripts/deploy/sync-secrets.sh ~/secrets/gearnest.env
```

The script:
- creates each secret if missing, otherwise adds a new version
- grants `roles/secretmanager.secretAccessor` to both service accounts
- skips empty values with a warning (so you can ship Phase 1 secrets first
  and add Phase 2 proxy creds later)

The allow-list (env-var → secret-name mapping) lives in `sync-secrets.sh`.
Any env var not in the list is **silently ignored** — secret promotion is an
explicit decision, not "everything in the file gets pushed."

---

## 6. Apply database migrations (one-time + on each schema change)

Cloud Run can't easily run migrations on boot (no sidecar pattern), so run
them from your laptop through the Cloud SQL Auth Proxy:

```bash
# In a separate terminal — leave running while you migrate.
cloud-sql-proxy gearnest-prod:us-central1:gearnest-db

# In your shell:
DATABASE_URL="postgres://gearnest@127.0.0.1:5432/gearnest" \
    (cd gear-nest-pipeline && cargo run -- migrate)
```

The Rust pipeline's `migrate` subcommand applies every
`supabase/migrations/*.sql` and runs idempotent partition DDL
(`price_history_YYYY_MM`). It's safe to re-run.

---

## 7. Build + push container images (idempotent — script)

```bash
# Build + push both api and pipeline images, tagged with current git SHA.
scripts/deploy/build-push-images.sh

# Or one at a time:
scripts/deploy/build-push-images.sh api
scripts/deploy/build-push-images.sh pipeline
```

Per [ADR-0024][adr24] we push **locally** rather than using Cloud Build —
one fewer GCP product to wire up. The script:
- builds `--platform linux/amd64` (Cloud Run is amd64; matters on Apple Silicon)
- tags with `IMAGE_TAG` (default: `git rev-parse --short HEAD`) **and** `latest`
- runs `gcloud auth configure-docker` first (no-op on subsequent runs)

After the build, the script prints the `IMAGE_TAG` value to use in §8 and §9.

---

## 8. Deploy the API to Cloud Run (idempotent — script)

```bash
export IMAGE_TAG=$(git rev-parse --short HEAD)
scripts/deploy/deploy-api.sh
```

What this wires:

| Setting | Value | Why |
|---|---|---|
| `--service-account` | `gearnest-api@…` | scoped IAM (Cloud SQL Client + Secret Accessor) |
| `--add-cloudsql-instances` | `gearnest-prod:us-central1:gearnest-db` | Cloud SQL Auth Proxy injection |
| `--allow-unauthenticated` | yes | public API |
| `--min-instances=0` | | $0 when idle (portfolio cost — cold start ~3s accepted) |
| `--max-instances=3` | | caps blast radius and bill exposure |
| `--cpu=1`, `--memory=512Mi` | | SPEC §15.2 service map |
| `--concurrency=40` | | SSE-friendly; Spring virtual threads handle the parallelism |
| `--set-secrets` | `REDIS_URL`, `HUGGINGFACE_API_KEY` | from Secret Manager |
| `WEB_ORIGIN` env var | `https://gearnest.app` | CORS allow-list for the Vercel web origin (override per deploy) |

The script prints the live service URL at the end (e.g.
`https://gearnest-api-XXXXXXXXXX-uc.a.run.app`) — paste that into the
Vercel project's `NEXT_PUBLIC_API_BASE_URL` (§10).

---

## 9. Deploy + schedule the pipeline jobs (idempotent — scripts)

```bash
export IMAGE_TAG=$(git rev-parse --short HEAD)
scripts/deploy/deploy-jobs.sh        # creates / updates the two Cloud Run Jobs
scripts/deploy/schedule-jobs.sh      # creates / updates the two Cloud Scheduler triggers
```

Per [ADR-0022][adr22] the pipeline is one-shot — `Cmd::PriceSync` and
`Cmd::FullSync` in `gear-nest-pipeline/src/main.rs` exit when done; Cloud
Scheduler fires them on cron.

Two jobs, two triggers:

| Job                   | Cron (UTC)   | Frequency | Timeout | Notes |
|-----------------------|--------------|-----------|---------|-------|
| `pipeline-price-sync` | `0 6 * * *`  | Daily     | 15 min  | 8-store price refresh (Redis SWR + `price_history`) |
| `pipeline-full-sync`  | `0 2 * * 0`  | Weekly    | 60 min  | full crawl + normalize + resolve + embed + upsert |

**Why UTC?** The schedule lives next to GCP infra and should not drift
across US daylight-saving transitions. Both windows are outside US peak
browsing (06:00 UTC = 23:00 US-Pacific previous day; 02:00 UTC Sunday =
19:00 US-Pacific Saturday), so a slow sync doesn't compete with API latency.

**Force-run a job** for smoke testing the schedule, without waiting for cron:

```bash
gcloud scheduler jobs run price-sync-daily --location us-central1
# or invoke the Job directly:
gcloud run jobs execute pipeline-price-sync --region us-central1 --wait
```

---

## 10. Deploy the web app on Vercel (manual, one-time)

GearNest web is hosted on Vercel (Hobby tier, free) with auto-deploy from
`main`. There's no script — Vercel's git integration does it.

1. https://vercel.com/import → import the `gear-nest` repo, set **Root
   Directory** to `gear-nest-web/`
2. Framework Preset: **Next.js** (auto-detected)
3. Environment Variables (set for **Production**, **Preview**, **Development**):
   - `NEXT_PUBLIC_API_BASE_URL` = the Cloud Run URL printed by §8
   - `NEXT_PUBLIC_USE_MOCKS` = `0`
4. Deploy

After the first deploy, redeploys happen on every push to `main` that
touches `gear-nest-web/**`. To rotate the API URL: change the Vercel env
var → trigger a redeploy (Vercel UI → Deployments → ⋯ → Redeploy).

If you map a custom domain (`gearnest.app`) in Vercel, update the API's
`WEB_ORIGIN`:

```bash
WEB_ORIGIN=https://gearnest.app scripts/deploy/deploy-api.sh
```

---

## 11. Smoke test the deployed stack

```bash
# Health (Spring Boot actuator — already wired in api/main)
curl -fsS "$API_URL/actuator/health"

# Live search against the seeded catalog
curl -fsS "$API_URL/api/v1/products/search?q=stove&limit=3" | jq '.results[].name'

# A chat stream (SSE) — verifies HuggingFace key + Redis session budget
curl -fsS -N \
    "$API_URL/api/v1/chat?query=best+lightweight+stove&productId=$(curl -s "$API_URL/api/v1/products/search?q=stove&limit=1" | jq -r '.results[0].id')"

# Trigger a one-off price sync (smoke the pipeline)
gcloud run jobs execute pipeline-price-sync --region us-central1 --wait
```

If the chat stream stalls before the first token, check:
- `HUGGINGFACE_API_KEY` secret has a recent **version** (not just exists)
- Cloud Run logs: `gcloud run services logs read gearnest-api --region us-central1 --limit 50`

---

## 12. Day-2 operations

| Need                                   | How                                                         |
|----------------------------------------|-------------------------------------------------------------|
| Re-deploy after code change            | `IMAGE_TAG=$(git rev-parse --short HEAD) scripts/deploy/build-push-images.sh && scripts/deploy/deploy-api.sh` |
| Rotate a secret                        | Update `../.env.production` → `scripts/deploy/sync-secrets.sh`. Cloud Run picks up new versions on next cold start; `gcloud run services update gearnest-api --region us-central1` forces revision rollover. |
| Schema change                          | Add migration under `supabase/migrations/` → repeat §6 with the Cloud SQL Auth Proxy |
| Pause everything overnight             | `gcloud run services update gearnest-api --region us-central1 --max-instances=0` (and pause Cloud Scheduler jobs); restore with `--max-instances=3` |
| Disaster: emergency stop               | Cloud Console → Billing → unlink billing account. Everything stops; data preserved for 30 days. Reverse by re-linking. |
| Verify CORS                            | `curl -I -H "Origin: https://gearnest.app" "$API_URL/api/v1/products/search?q=x"` → expect `Access-Control-Allow-Origin: https://gearnest.app` |
| Watch costs                            | Cloud Console → Billing → Reports; daily-spend Cloud Monitoring alert is recommended (SPEC §15.5) |

---

## Reference: file map

| Script                                | What it does                                                |
|---------------------------------------|-------------------------------------------------------------|
| `scripts/deploy/lib.sh`               | Shared config + helpers; sourced, not executed              |
| `scripts/deploy/bootstrap-gcp.sh`     | APIs, Artifact Registry, SAs, IAM, Cloud SQL, DB, IAM users |
| `scripts/deploy/sync-secrets.sh`      | `.env.production` → Secret Manager (allow-listed)           |
| `scripts/deploy/build-push-images.sh` | Build + push api / pipeline images (linux/amd64)            |
| `scripts/deploy/deploy-api.sh`        | Create or update Cloud Run service `gearnest-api`           |
| `scripts/deploy/deploy-jobs.sh`       | Create or update Cloud Run Jobs (price-sync, full-sync)     |
| `scripts/deploy/schedule-jobs.sh`     | Create or update Cloud Scheduler triggers (UTC)             |

Every script is `set -euo pipefail` and idempotent: re-running is safe.
Override defaults via env (`GCP_PROJECT`, `GCP_REGION`, …) — see `lib.sh`.
