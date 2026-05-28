//! Three-tier entity resolution (ADR-007).
//!
//! Given a normalized scrape from store X, decide whether it matches an
//! existing canonical `products` row or should create one. The output is a
//! [`crate::models::ResolvedProduct`] carrying the `product_id` and a
//! confidence label. `CANDIDATE` rows are written to `store_listings` but
//! filtered from the user-facing UI; they sit in a review queue.
//!
//! * **Tier 1 — GTIN / ASIN.** O(1) exact match via the `products.gtin` index
//!   or via `store_listings.store_product_id`. EXACT confidence.
//! * **Tier 2 — structured attributes.** Brand alias (curated) + extracted
//!   model token combine into `canonical_key = "<brand>:<model>"`. Match
//!   produces HIGH confidence.
//! * **Tier 3 — embedding similarity.** Cosine over the canonical attribute
//!   string against existing `products` embeddings. MEDIUM if ≥ 0.92,
//!   CANDIDATE if 0.80..0.92, otherwise we create a new product.
//!
//! Tier 3 requires an external embedding model to be wired (see
//! [`crate::embeddings`]); this module exposes the entrypoint and the
//! candidate-similarity scoring helper but leaves the actual vector lookup as
//! a deliberate skeleton for Phase 1.

use anyhow::Result;
use sqlx::postgres::PgPool;
use tracing::{debug, instrument};
use uuid::Uuid;

use crate::models::{MatchConfidence, NormalizedProduct, RawProduct, ResolvedProduct};

const TIER3_HIGH_THRESHOLD: f32 = 0.92;
const TIER3_CANDIDATE_THRESHOLD: f32 = 0.80;

pub struct Resolver<'a> {
    pool: &'a PgPool,
}

impl<'a> Resolver<'a> {
    pub fn new(pool: &'a PgPool) -> Self {
        Self { pool }
    }

    /// Resolve a normalized product, creating a `products` row when no match
    /// is found, and always writing the `store_listings` row tagged with the
    /// confidence tier.
    #[instrument(skip(self, raw, norm), fields(store = %raw.store_id, asin = %raw.store_product_id))]
    pub async fn resolve(
        &self,
        raw: &RawProduct,
        norm: &NormalizedProduct,
    ) -> Result<ResolvedProduct> {
        // ─── Tier 1: GTIN / ASIN identity ─────────────────────────────────
        if let Some(gtin) = norm.gtin.as_deref() {
            if !gtin.is_empty() {
                if let Some(existing) = lookup_by_gtin(self.pool, gtin).await? {
                    let confidence = MatchConfidence::Exact;
                    upsert_store_listing(self.pool, existing, raw, confidence).await?;
                    debug!(tier = 1, %existing, "matched by gtin");
                    return Ok(ResolvedProduct {
                        product_id: existing,
                        created: false,
                        confidence,
                    });
                }
            }
        }
        if let Some(existing) =
            lookup_by_store_listing(self.pool, &raw.store_id, &raw.store_product_id).await?
        {
            let confidence = MatchConfidence::Exact;
            upsert_store_listing(self.pool, existing, raw, confidence).await?;
            debug!(tier = 1, %existing, "matched by store listing identity");
            return Ok(ResolvedProduct {
                product_id: existing,
                created: false,
                confidence,
            });
        }

        // ─── Tier 2: canonical_key (brand + model) ────────────────────────
        if !norm.canonical_key.is_empty() {
            if let Some(existing) = lookup_by_canonical_key(self.pool, &norm.canonical_key).await? {
                let confidence = MatchConfidence::High;
                upsert_store_listing(self.pool, existing, raw, confidence).await?;
                debug!(tier = 2, %existing, "matched by canonical_key");
                return Ok(ResolvedProduct {
                    product_id: existing,
                    created: false,
                    confidence,
                });
            }
        }

        // ─── Tier 3: embedding similarity (skeleton) ──────────────────────
        // The full implementation hangs off `crate::embeddings::similar_products`
        // and is exercised in Phase 2 once cross-store coverage exists. For now
        // we always fall through to product creation with EXACT confidence
        // (this listing is the only one we've seen for this product).
        let new_product_id = create_product(self.pool, norm).await?;
        let confidence = MatchConfidence::Exact;
        upsert_store_listing(self.pool, new_product_id, raw, confidence).await?;
        Ok(ResolvedProduct {
            product_id: new_product_id,
            created: true,
            confidence,
        })
    }
}

/// Tier-3 helper: classify a cosine similarity into a confidence bucket.
/// Pure function so the threshold table is unit-testable without a DB.
pub fn classify_similarity(cosine: f32) -> Option<MatchConfidence> {
    if cosine >= TIER3_HIGH_THRESHOLD {
        Some(MatchConfidence::Medium)
    } else if cosine >= TIER3_CANDIDATE_THRESHOLD {
        Some(MatchConfidence::Candidate)
    } else {
        None
    }
}

async fn lookup_by_gtin(pool: &PgPool, gtin: &str) -> Result<Option<Uuid>> {
    let row: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM products WHERE gtin = $1 LIMIT 1")
        .bind(gtin)
        .fetch_optional(pool)
        .await?;
    Ok(row.map(|(id,)| id))
}

async fn lookup_by_canonical_key(pool: &PgPool, key: &str) -> Result<Option<Uuid>> {
    let row: Option<(Uuid,)> =
        sqlx::query_as("SELECT id FROM products WHERE canonical_key = $1 LIMIT 1")
            .bind(key)
            .fetch_optional(pool)
            .await?;
    Ok(row.map(|(id,)| id))
}

async fn lookup_by_store_listing(
    pool: &PgPool,
    store_id: &str,
    store_product_id: &str,
) -> Result<Option<Uuid>> {
    let row: Option<(Uuid,)> = sqlx::query_as(
        "SELECT product_id FROM store_listings \
         WHERE store_id = $1 AND store_product_id = $2 LIMIT 1",
    )
    .bind(store_id)
    .bind(store_product_id)
    .fetch_optional(pool)
    .await?;
    Ok(row.map(|(id,)| id))
}

async fn create_product(pool: &PgPool, norm: &NormalizedProduct) -> Result<Uuid> {
    let row: (Uuid,) = sqlx::query_as(
        r"
        INSERT INTO products
            (slug, name, brand, category, subcategory, description, specs,
             primary_image, gtin, canonical_key)
        VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, NULLIF($10, ''))
        ON CONFLICT (slug) DO UPDATE
            SET name           = EXCLUDED.name,
                brand          = EXCLUDED.brand,
                category       = EXCLUDED.category,
                subcategory    = EXCLUDED.subcategory,
                description    = COALESCE(EXCLUDED.description, products.description),
                specs          = products.specs || EXCLUDED.specs,
                primary_image  = COALESCE(EXCLUDED.primary_image, products.primary_image),
                updated_at     = NOW()
        RETURNING id
        ",
    )
    .bind(&norm.slug)
    .bind(&norm.name)
    .bind(&norm.brand)
    .bind(&norm.category)
    .bind(norm.subcategory.as_deref())
    .bind(norm.description.as_deref())
    .bind(&norm.specs)
    .bind(norm.primary_image.as_deref())
    .bind(norm.gtin.as_deref())
    .bind(&norm.canonical_key)
    .fetch_one(pool)
    .await?;
    Ok(row.0)
}

async fn upsert_store_listing(
    pool: &PgPool,
    product_id: Uuid,
    raw: &RawProduct,
    confidence: MatchConfidence,
) -> Result<()> {
    sqlx::query(
        r"
        INSERT INTO store_listings
            (product_id, store_id, store_product_id, store_url,
             store_rating, store_review_count, match_confidence, last_synced_at)
        VALUES ($1, $2, $3, $4, $5, $6, $7, NOW())
        ON CONFLICT (store_id, store_product_id) DO UPDATE
            SET product_id         = EXCLUDED.product_id,
                store_url          = EXCLUDED.store_url,
                store_rating       = EXCLUDED.store_rating,
                store_review_count = EXCLUDED.store_review_count,
                match_confidence   = EXCLUDED.match_confidence,
                last_synced_at     = NOW()
        ",
    )
    .bind(product_id)
    .bind(&raw.store_id)
    .bind(&raw.store_product_id)
    .bind(&raw.url)
    .bind(raw.store_rating)
    .bind(raw.store_review_count)
    .bind(confidence.as_db_str())
    .execute(pool)
    .await?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn similarity_classification_table() {
        assert_eq!(classify_similarity(0.95), Some(MatchConfidence::Medium));
        assert_eq!(classify_similarity(0.92), Some(MatchConfidence::Medium));
        assert_eq!(classify_similarity(0.85), Some(MatchConfidence::Candidate));
        assert_eq!(classify_similarity(0.80), Some(MatchConfidence::Candidate));
        assert_eq!(classify_similarity(0.50), None);
    }
}
