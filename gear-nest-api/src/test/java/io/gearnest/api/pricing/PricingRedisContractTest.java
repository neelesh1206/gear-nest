package io.gearnest.api.pricing;

import io.gearnest.api.product.dto.PriceComparisonResponse;
import io.gearnest.api.product.dto.StoreListing;
import io.gearnest.api.support.AbstractIntegrationTest;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.jdbc.core.JdbcTemplate;

import java.time.OffsetDateTime;
import java.time.ZoneOffset;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;

// Locks the Redis price contract between pipeline (writer) and API (reader).
// The pipeline writes key `prices:{product_id}`, field `{store_id}`, value JSON
// {listing_id, price, in_stock, fetched_at, jitter_secs} — see
// docs/contracts/redis-schema.md. A prior divergence (API read price:listing:{id}
// flat fields) silently broke the cache layer; this test pins the format.
class PricingRedisContractTest extends AbstractIntegrationTest {

    @Autowired PricingService pricingService;
    @Autowired JdbcTemplate jdbc;
    @Autowired StringRedisTemplate redis;

    @Test
    void readsLivePriceWrittenInPipelineFormat() {
        UUID productId = UUID.randomUUID();
        UUID listingId = UUID.randomUUID();

        jdbc.update("INSERT INTO products (id, slug, name, brand, category) VALUES (?, ?, ?, ?, ?)",
            productId, "stove-" + productId, "Test Stove", "MSR", "camp-kitchen");
        jdbc.update("""
            INSERT INTO store_listings
              (id, product_id, store_id, store_product_id, store_url, store_rating, store_review_count, match_confidence)
            VALUES (?, ?, 'amazon', 'B0TEST', 'https://amazon.com/x', 4.4, 312, 'EXACT')
            """, listingId, productId);

        String fetchedAt = OffsetDateTime.now(ZoneOffset.UTC).toString();
        String payload = ("{\"listing_id\":\"" + listingId + "\",\"price\":\"129.99\","
            + "\"in_stock\":true,\"fetched_at\":\"" + fetchedAt + "\",\"jitter_secs\":0}");
        redis.opsForHash().put("prices:" + productId, "amazon", payload);

        PriceComparisonResponse resp = pricingService.comparisonFor(productId);

        assertThat(resp.listings()).hasSize(1);
        StoreListing listing = resp.listings().get(0);
        assertThat(listing.store().id()).isEqualTo("amazon");
        assertThat(listing.price()).isEqualTo(129.99f);
        assertThat(listing.inStock()).isTrue();
        assertThat(listing.isStale()).isFalse();
    }

    // Reader must honor jitter_secs in the staleness rule (24h + jitter), matching
    // the pipeline's PricePayload::is_stale. A price fetched 24h30m ago with a
    // 60-min jitter is still fresh; with zero jitter it would be stale.
    @Test
    void honorsJitterInStalenessRule() {
        UUID productId = UUID.randomUUID();
        UUID listingId = UUID.randomUUID();

        jdbc.update("INSERT INTO products (id, slug, name, brand, category) VALUES (?, ?, ?, ?, ?)",
            productId, "stove-" + productId, "Test Stove", "MSR", "camp-kitchen");
        jdbc.update("""
            INSERT INTO store_listings
              (id, product_id, store_id, store_product_id, store_url, store_rating, store_review_count, match_confidence)
            VALUES (?, ?, 'rei', 'R0TEST', 'https://rei.com/x', 4.4, 50, 'EXACT')
            """, listingId, productId);

        String fetchedAt = OffsetDateTime.now(ZoneOffset.UTC).minusHours(24).minusMinutes(30).toString();
        String payload = ("{\"listing_id\":\"" + listingId + "\",\"price\":\"59.00\","
            + "\"in_stock\":true,\"fetched_at\":\"" + fetchedAt + "\",\"jitter_secs\":3600}");
        redis.opsForHash().put("prices:" + productId, "rei", payload);

        StoreListing listing = pricingService.comparisonFor(productId).listings().get(0);
        assertThat(listing.price()).isEqualTo(59.00f);
        assertThat(listing.isStale()).isFalse();
    }
}
