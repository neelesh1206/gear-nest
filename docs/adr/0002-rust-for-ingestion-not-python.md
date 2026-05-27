# ADR-002: Rust for ingestion, not Python

**Status:** Accepted
**Date:** 2026-05-27

## Decision
Rust handles all scraping, normalization, chunking, and embedding calls.

## Rationale
Portfolio signal (demonstrates Rust in a real-world pipeline). Technically justified: concurrent async I/O with Tokio is more memory-efficient than Python async for 8 simultaneous store scrapers. The `scraper` crate provides CSS-selector-based HTML parsing comparable to BeautifulSoup with zero GC overhead.

## Trade-off
Rust has a steeper iteration cycle than Python Scrapy. Accepted cost for the portfolio value.
