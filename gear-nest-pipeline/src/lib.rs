//! `GearNest` ingestion pipeline.
//!
//! Library entry point. Modules:
//!
//! * [`config`]            — environment-driven configuration.
//! * [`db`]                — Postgres pool + migration runner.
//! * [`scrapers`]          — per-store crawlers behind a common trait.
//! * [`normalizer`]        — title / category / brand canonicalization.
//! * [`entity_resolution`] — three-tier product matching (ADR-007).
//! * [`embeddings`]        — `HuggingFace` batched embed → pgvector bulk insert.
//! * [`prices`]            — stale-while-revalidate Redis writer (ADR-009).
//! * [`price_history`]     — partition-aware append + idempotent DDL (ADR-010).
//! * [`reviews`]           — idempotent review persistence (SPEC §13, Phase 3).

pub mod config;
pub mod db;
pub mod dedup_reviews;
pub mod embeddings;
pub mod entity_resolution;
pub mod full_sync;
pub mod models;
pub mod normalizer;
pub mod price_history;
pub mod price_sync;
pub mod prices;
pub mod reviews;
pub mod scrapers;
pub mod sync_reviews;
