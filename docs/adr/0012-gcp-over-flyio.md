# ADR-012: GCP (Cloud Run + Cloud SQL) over Fly.io

**Status:** Accepted
**Date:** 2026-05-27

## Decision
Java API runs on Cloud Run; PostgreSQL runs on Cloud SQL. Fly.io dropped. Redis stays on Upstash (not GCP Memorystore).

## Rationale
GCP signals enterprise cloud fluency directly relevant to Staff SWE roles at Walmart and peers — IAM/Workload Identity, VPC private networking, Secret Manager, Cloud Monitoring are all patterns used in production at F500 companies. Fly.io is excellent DX but sends a startup/indie signal, not an enterprise one. Memorystore excluded because its minimum instance cost (~$35/month) alone would exceed the $30/month project budget cap; Upstash free tier is functionally equivalent at portfolio scale.

## Trade-off
Significantly more setup complexity than Fly.io (IAM policies, VPC connectors, Workload Identity Federation for GitHub Actions). Accepted — the setup work itself is portfolio signal. Estimated monthly cost: $19–20, well under the $30 hard cap enforced by the billing auto-stop Cloud Function (SPEC §15.5).
