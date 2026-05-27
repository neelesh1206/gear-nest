# ADR-004: BAAI/bge-small-en-v1.5 for embeddings

**Status:** Accepted
**Date:** 2026-05-27

## Decision
384-dimension embeddings from `BAAI/bge-small-en-v1.5`.

## Rationale
Outperforms `all-MiniLM-L6-v2` on MTEB retrieval benchmarks. 384 dimensions vs 1536 (OpenAI ada-002) → 4× smaller vectors → 4× smaller index → faster search, cheaper storage. Free via HuggingFace Inference API.

## Trade-off
Slightly lower quality than OpenAI `text-embedding-3-large` on complex queries. Acceptable for product/review retrieval.
