-- GearNest initial schema
-- Source: SPEC.md §6 (Data Model)
-- Owner: Session 0 (Contract). Sessions A/B/C must NOT alter this file;
-- schema changes go in subsequent numbered migrations after Session 0 review.

CREATE EXTENSION IF NOT EXISTS vector;
CREATE EXTENSION IF NOT EXISTS pg_trgm;
CREATE EXTENSION IF NOT EXISTS pgcrypto;

-- ---------------------------------------------------------------------------
-- products: canonical store-agnostic product record
-- ---------------------------------------------------------------------------
CREATE TABLE products (
    id              UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    slug            TEXT UNIQUE NOT NULL,
    name            TEXT NOT NULL,
    brand           TEXT NOT NULL,
    category        TEXT NOT NULL,
    subcategory     TEXT,
    description     TEXT,
    specs           JSONB,
    primary_image   TEXT,
    gtin            TEXT,
    canonical_key   TEXT,
    created_at      TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    updated_at      TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE INDEX products_brand_idx       ON products (brand);
CREATE INDEX products_category_idx    ON products (category, subcategory);
CREATE INDEX products_gtin_idx        ON products (gtin) WHERE gtin IS NOT NULL;
CREATE INDEX products_canonical_idx   ON products (canonical_key) WHERE canonical_key IS NOT NULL;
CREATE INDEX products_fts_idx         ON products
    USING GIN (to_tsvector('english', name || ' ' || brand || ' ' || COALESCE(description, '')));

-- ---------------------------------------------------------------------------
-- stores: registry
-- ---------------------------------------------------------------------------
CREATE TABLE stores (
    id              TEXT PRIMARY KEY,
    display_name    TEXT NOT NULL,
    base_url        TEXT NOT NULL,
    logo_url        TEXT,
    affiliate_type  TEXT,
    active          BOOLEAN NOT NULL DEFAULT TRUE
);

-- ---------------------------------------------------------------------------
-- store_listings: per-product per-store static metadata.
-- Mutable price/stock live in Redis (ADR-006). No price column here.
-- ---------------------------------------------------------------------------
CREATE TABLE store_listings (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id          UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    store_id            TEXT NOT NULL REFERENCES stores(id),
    store_product_id    TEXT NOT NULL,
    store_url           TEXT NOT NULL,
    affiliate_url       TEXT,
    store_rating        NUMERIC(3,2),
    store_review_count  INT NOT NULL DEFAULT 0,
    match_confidence    TEXT NOT NULL DEFAULT 'EXACT'
                        CHECK (match_confidence IN ('EXACT','HIGH','MEDIUM','CANDIDATE')),
    last_synced_at      TIMESTAMPTZ,
    UNIQUE (store_id, store_product_id)
);

CREATE INDEX store_listings_product_idx     ON store_listings (product_id);
CREATE INDEX store_listings_confidence_idx  ON store_listings (match_confidence);

-- ---------------------------------------------------------------------------
-- price_history: append-only, range-partitioned by month on fetched_at.
-- Per ADR-010: monthly partitions keep indexes resident in buffer pool;
-- retention = DROP PARTITION (O(1)).
-- ---------------------------------------------------------------------------
CREATE TABLE price_history (
    id          BIGSERIAL,
    listing_id  UUID NOT NULL,
    price       NUMERIC(10,2) NOT NULL,
    in_stock    BOOLEAN,
    fetched_at  TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (id, fetched_at)
) PARTITION BY RANGE (fetched_at);

CREATE INDEX price_history_listing_idx ON price_history (listing_id, fetched_at DESC);

-- Seed partitions. Rust pipeline creates subsequent months at startup (idempotent DDL).
CREATE TABLE price_history_2026_05 PARTITION OF price_history
    FOR VALUES FROM ('2026-05-01') TO ('2026-06-01');
CREATE TABLE price_history_2026_06 PARTITION OF price_history
    FOR VALUES FROM ('2026-06-01') TO ('2026-07-01');
CREATE TABLE price_history_2026_07 PARTITION OF price_history
    FOR VALUES FROM ('2026-07-01') TO ('2026-08-01');

-- ---------------------------------------------------------------------------
-- reviews: individual reviews scraped per store
-- ---------------------------------------------------------------------------
CREATE TABLE reviews (
    id                  UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id          UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    store_id            TEXT NOT NULL REFERENCES stores(id),
    source_review_id    TEXT,
    reviewer_id_hash    TEXT,
    rating              SMALLINT NOT NULL CHECK (rating BETWEEN 1 AND 5),
    title               TEXT,
    body                TEXT NOT NULL,
    verified_purchase   BOOLEAN NOT NULL DEFAULT FALSE,
    helpful_votes       INT NOT NULL DEFAULT 0,
    review_date         DATE,
    scraped_at          TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE (store_id, source_review_id)
);

CREATE INDEX reviews_product_idx        ON reviews (product_id);
CREATE INDEX reviews_product_rating_idx ON reviews (product_id, rating);
CREATE INDEX reviews_reviewer_hash_idx  ON reviews (reviewer_id_hash) WHERE reviewer_id_hash IS NOT NULL;

-- ---------------------------------------------------------------------------
-- review_chunks: pgvector-indexed semantic chunks.
-- NO HNSW (ADR-001). Chat is product-scoped; exact KNN over ~75 rows is faster
-- than a 99.98%-filtered HNSW graph traversal.
-- ---------------------------------------------------------------------------
CREATE TABLE review_chunks (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    review_id   UUID NOT NULL REFERENCES reviews(id) ON DELETE CASCADE,
    product_id  UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    chunk_text  TEXT NOT NULL,
    chunk_index SMALLINT NOT NULL,
    embedding   vector(384),
    rating      SMALLINT,
    store_id    TEXT
);

CREATE INDEX review_chunks_product_idx        ON review_chunks (product_id);
CREATE INDEX review_chunks_product_rating_idx ON review_chunks (product_id, rating);

-- ---------------------------------------------------------------------------
-- spec_chunks: pgvector-indexed product spec/description chunks
-- ---------------------------------------------------------------------------
CREATE TABLE spec_chunks (
    id          UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    product_id  UUID NOT NULL REFERENCES products(id) ON DELETE CASCADE,
    chunk_text  TEXT NOT NULL,
    chunk_index SMALLINT NOT NULL,
    source_type TEXT NOT NULL CHECK (source_type IN ('description', 'specs', 'features')),
    embedding   vector(384)
);

CREATE INDEX spec_chunks_product_idx ON spec_chunks (product_id);

-- ---------------------------------------------------------------------------
-- ai_summaries: cached AI-generated product summaries, invalidated on review delta
-- ---------------------------------------------------------------------------
CREATE TABLE ai_summaries (
    product_id      UUID PRIMARY KEY REFERENCES products(id) ON DELETE CASCADE,
    summary_text    TEXT NOT NULL,
    pros            TEXT[],
    cons            TEXT[],
    review_count    INT NOT NULL,
    generated_at    TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

-- ---------------------------------------------------------------------------
-- Seed stores registry
-- ---------------------------------------------------------------------------
INSERT INTO stores (id, display_name, base_url, affiliate_type) VALUES
    ('amazon',         'Amazon',             'https://www.amazon.com',          'pa-api'),
    ('rei',            'REI Co-op',          'https://www.rei.com',             'cj'),
    ('backcountry',    'Backcountry',        'https://www.backcountry.com',     'scrape'),
    ('cabelas',        'Cabela''s',          'https://www.cabelas.com',         'scrape'),
    ('moosejaw',       'Moosejaw',           'https://www.moosejaw.com',        'scrape'),
    ('steepandcheap',  'Steep & Cheap',      'https://www.steepandcheap.com',   'scrape'),
    ('campsaver',      'CampSaver',          'https://www.campsaver.com',       'scrape'),
    ('garagerowngear', 'Garage Grown Gear',  'https://www.garagegrowngear.com', 'scrape');
