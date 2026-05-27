# ADR-001: pgvector over a dedicated vector database

**Status:** Accepted
**Date:** 2026-05-27

## Decision
Use PostgreSQL + pgvector extension rather than Pinecone, Weaviate, or Qdrant.

## Rationale
Products, reviews, prices, and vectors are all relational. Keeping them in one database eliminates cross-service joins and consistency complexity. Because chat is always product-scoped, we use exact KNN over ~75 rows per product — no HNSW needed on chunk tables, and no dedicated vector DB performance advantage applies at this scope. A dedicated vector DB adds operational overhead with no benefit.

## Trade-off
If chat scope ever expands to cross-product queries over 8M+ vectors, add HNSW then — or revisit Qdrant. But YAGNI.
