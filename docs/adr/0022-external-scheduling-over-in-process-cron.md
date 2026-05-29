# ADR-0022: External scheduling (Cloud Scheduler → one-shot Cloud Run Job) over in-process cron

**Status:** Accepted
**Date:** 2026-05-29
**Owner:** Session 0 (contract track)

## Context
The pipeline runs on cadences (SPEC §5/§7): **daily price sync** (~06:00 UTC) and
**weekly full product sync** (Sun ~02:00 UTC), with daily review sync in Phase 3.
SPEC §7 mentions `tokio-cron-scheduler` (an in-process scheduler), but SPEC §15.2/§15.6
deploys the pipeline as **Cloud Run Jobs triggered by Cloud Scheduler**. These two
models conflict, and the choice affects both the pipeline's shape and the cost.

## Decision
Schedule the pipeline **externally**: GCP Cloud Scheduler fires a **one-shot Cloud
Run Job** that runs the pipeline binary (`pipeline price-sync` / `full-sync`) to
completion and exits. The pipeline stays a one-shot CLI — **no in-process
`tokio-cron-scheduler`, no long-lived daemon.**

- Phase 2 builds a one-shot `price-sync` subcommand (iterate active listings across
  the 8 stores → Redis SWR + `price_history`, per-store `governor` rate limits).
- Phase 5 (deploy) wires Cloud Scheduler crons: `0 6 * * *` (daily price) and
  `0 2 * * 0` (weekly full). Local dev runs the subcommand manually (or host cron).

## Rationale
- **Cost (the $30/mo cap, ADR-012):** Cloud Run Jobs are ephemeral (~$1/mo — spin up,
  run, exit). An in-process scheduler needs an always-on container (min-instances ≥1)
  just to sleep until the cron time — wasteful for a job that runs minutes per day.
- **Simplicity / reliability:** one-shot runs have no long-lived state, restart
  cleanly, and the schedule lives in one declarative place (Cloud Scheduler) rather
  than embedded in app code. Failures are visible as failed Job executions.
- **Cloud Scheduler is free-tier** (3 jobs) on GCP.

## Trade-off
Local "set-and-forget" daily runs need a host `cron`/launchd entry rather than the
binary self-scheduling. Acceptable — local dev typically runs syncs on demand. If a
non-GCP, always-on deployment is ever needed, `tokio-cron-scheduler` can be added
behind a `--daemon` flag without disturbing the one-shot subcommands.
