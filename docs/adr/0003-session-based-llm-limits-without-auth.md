# ADR-003: Session-based LLM limits without auth

**Status:** Accepted
**Date:** 2026-05-27

## Decision
5 questions per 2-hour session tracked via Redis, no user accounts.

## Rationale
Auth creates friction that kills casual discovery. HuggingFace Pro rate limits need managing. Cookie sessions are transparent to the user and reset naturally. The limit is shown in the UI with a "why" tooltip.

## Trade-off
Users who want unlimited access have no path to it in v1. Accepted — this is about the community, not power users.
