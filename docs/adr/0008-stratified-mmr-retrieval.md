# ADR-008: Stratified + MMR retrieval over pure top-K cosine

**Status:** Accepted
**Date:** 2026-05-27

## Decision
RAG retrieval uses semantic top-5 (MMR, λ=0.7) + top-3 negative review chunks (rating ≤ 2) + top-2 spec chunks, rather than pure top-10 cosine similarity.

## Rationale
Pure top-K on a directional query ("Is this good for rain?") clusters retrieved chunks around the query topic and systematically excludes failure modes. A user asking about rain performance deserves to see the 3 reviews saying "failed in snow" — that's what makes the answer trustworthy. Stratified retrieval guarantees balanced sentiment at the cost of two additional filtered queries. MMR on the semantic slots ensures topical diversity rather than returning near-duplicates.

## Trade-off
Slightly more complex retrieval logic in `RagService`. Worth it for answer quality and user trust.
