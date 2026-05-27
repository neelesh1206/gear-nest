# ADR-005: Hybrid search (vector + FTS) for catalog

**Status:** Accepted
**Date:** 2026-05-27

## Decision
Product catalog search uses weighted combination of pgvector cosine similarity and PostgreSQL full-text search: `0.6 × vector + 0.4 × FTS`.

## Rationale
Semantic search alone ranks "lightweight waterproof jacket" well but struggles with exact brand/model queries ("Arc'teryx Beta AR"). FTS handles exact matches. Weighted combination covers both use cases.

## Trade-off
Two scoring components must be tuned together. The 0.6/0.4 split is a starting point; revisit with real query logs.
