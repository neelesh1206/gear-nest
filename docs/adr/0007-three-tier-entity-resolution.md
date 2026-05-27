# ADR-007: Three-tier entity resolution with confidence gating

**Status:** Accepted
**Date:** 2026-05-27

## Decision
Cross-store product matching uses GTIN/ASIN (Exact) → structured attribute extraction (High) → embedding similarity (Medium/Candidate), with `CANDIDATE` rows excluded from user-facing UI.

## Rationale
Naive string similarity on raw product titles produces massive false positives. "MSR PocketRocket 2" ≠ "Mountain Safety Research Pocket Rocket II Stove" under edit-distance metrics, yet they are the same product. Structured attribute normalization (brand alias + model regex) catches the majority deterministically. Confidence gating prevents silent data corruption: uncertain matches go to a review queue, never to the price comparison table.

## Trade-off
Tier 2 requires maintaining a brand alias dictionary. ~200 outdoor + fitness brands is manageable. Accuracy on niche cottage brands (Garage Grown Gear exclusives) will be lower — acceptable since those products rarely appear on other stores.
