# ADR-011: MinHash Stage 2 gated at 150-char review body length

**Status:** Accepted
**Date:** 2026-05-27

## Decision
Stage 2 near-deduplication (MinHash LSH) only runs on reviews with body length ≥ 150 characters.

## Rationale
3-gram word shingles on short reviews ("Great product, fast shipping!") produce near-identical token sets across distinct authentic reviews, causing false-positive deduplication. Stage 2 is designed to catch multi-paragraph syndicated corporate content — not short genuine reviews. Reviews under 150 chars are already handled by `UNIQUE(store_id, source_review_id)` exact dedup.

## Trade-off
Short duplicate reviews across stores won't be detected. Acceptable — short cross-store review duplication is rare and preserving authentic short reviews is more important than eliminating the edge case.
