package io.gearnest.api.reviews;

import io.gearnest.api.product.dto.ReviewBreakdown;
import io.gearnest.api.support.AbstractIntegrationTest;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.web.client.TestRestTemplate;
import org.springframework.boot.test.web.server.LocalServerPort;
import org.springframework.http.HttpStatus;
import org.springframework.http.ResponseEntity;
import org.springframework.jdbc.core.namedparam.MapSqlParameterSource;
import org.springframework.jdbc.core.namedparam.NamedParameterJdbcTemplate;

import java.time.LocalDate;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;

class ReviewEndpointIntegrationTest extends AbstractIntegrationTest {

    @LocalServerPort int port;
    @Autowired NamedParameterJdbcTemplate jdbc;
    @Autowired TestRestTemplate http;

    @BeforeEach
    void clean() {
        jdbc.update("DELETE FROM ai_summaries", new MapSqlParameterSource());
        jdbc.update("DELETE FROM review_chunks", new MapSqlParameterSource());
        jdbc.update("DELETE FROM reviews", new MapSqlParameterSource());
        jdbc.update("DELETE FROM store_listings", new MapSqlParameterSource());
        jdbc.update("DELETE FROM products", new MapSqlParameterSource());
    }

    @Test
    void noTierParamReturnsAllFiveTiersWithRankedSamples() {
        UUID productId = insertProduct("nemo-dagger", "NEMO Dagger 2P");
        insertListing(productId, "amazon", "B07X", "EXACT", 4.7f, 200);
        insertListing(productId, "rei", "REI1", "EXACT", 4.6f, 80);

        // 5-star tier: verified should beat unverified even with fewer
        // helpful_votes (SPEC §13 ranking).
        UUID verified5 = insertReview(productId, "amazon", 5, true, 3, LocalDate.of(2025, 6, 1));
        UUID unverified5 = insertReview(productId, "rei", 5, false, 99, LocalDate.of(2025, 9, 1));
        // 4-star tier: both unverified, helpful_votes tiebreak.
        UUID hi4 = insertReview(productId, "amazon", 4, false, 50, LocalDate.of(2025, 7, 1));
        UUID lo4 = insertReview(productId, "rei", 4, false, 10, LocalDate.of(2025, 8, 1));
        // 1-star tier: single review, surfaces alone.
        UUID solo1 = insertReview(productId, "campsaver", 1, false, 0, LocalDate.of(2025, 5, 1));

        ResponseEntity<ReviewBreakdown> resp = http.getForEntity(
            url("/api/v1/products/" + productId + "/reviews"),
            ReviewBreakdown.class);

        assertThat(resp.getStatusCode()).isEqualTo(HttpStatus.OK);
        ReviewBreakdown body = resp.getBody();
        assertThat(body).isNotNull();
        assertThat(body.total()).isEqualTo(5);
        assertThat(body.tiers()).containsKeys("1", "2", "3", "4", "5");

        // 5★: count=2, verified is the first sample.
        assertThat(body.tiers().get("5").count()).isEqualTo(2);
        assertThat(body.tiers().get("5").sample())
            .extracting("id")
            .containsExactly(verified5, unverified5);

        // 4★: count=2, helpful_votes orders unverified pair.
        assertThat(body.tiers().get("4").count()).isEqualTo(2);
        assertThat(body.tiers().get("4").sample())
            .extracting("id")
            .containsExactly(hi4, lo4);

        // 1★: single review surfaces; 2★/3★ empty samples.
        assertThat(body.tiers().get("1").sample())
            .extracting("id").containsExactly(solo1);
        assertThat(body.tiers().get("2").count()).isZero();
        assertThat(body.tiers().get("2").sample()).isEmpty();
        assertThat(body.tiers().get("3").count()).isZero();

        // storeBreakdown carries per-store aggregates from store_listings
        // (excluding CANDIDATE per ADR-007), highest review_count first.
        assertThat(body.storeBreakdown()).hasSize(2);
        assertThat(body.storeBreakdown().get(0).store().id()).isEqualTo("amazon");
        assertThat(body.storeBreakdown().get(0).count()).isEqualTo(200);
        assertThat(body.storeBreakdown().get(0).avgRating()).isEqualTo(4.7f);
    }

    @Test
    void tierQueryReturnsOnlyThatTierPaginated() {
        UUID productId = insertProduct("msr-hubba", "MSR Hubba Hubba NX");
        // Seed five 5-star reviews so pagination is meaningful.
        UUID r1 = insertReview(productId, "amazon", 5, true, 30, LocalDate.of(2025, 9, 1));
        UUID r2 = insertReview(productId, "amazon", 5, true, 20, LocalDate.of(2025, 8, 1));
        UUID r3 = insertReview(productId, "amazon", 5, false, 100, LocalDate.of(2025, 7, 1));
        UUID r4 = insertReview(productId, "rei", 5, false, 50, LocalDate.of(2025, 6, 1));
        UUID r5 = insertReview(productId, "rei", 5, false, 10, LocalDate.of(2025, 5, 1));

        ResponseEntity<ReviewBreakdown> page1 = http.getForEntity(
            url("/api/v1/products/" + productId + "/reviews?tier=5&page=1&size=2"),
            ReviewBreakdown.class);
        assertThat(page1.getStatusCode()).isEqualTo(HttpStatus.OK);
        assertThat(page1.getBody()).isNotNull();
        assertThat(page1.getBody().tiers()).containsOnlyKeys("5");
        assertThat(page1.getBody().tiers().get("5").count()).isEqualTo(5);
        assertThat(page1.getBody().tiers().get("5").sample())
            .extracting("id").containsExactly(r1, r2);

        ResponseEntity<ReviewBreakdown> page2 = http.getForEntity(
            url("/api/v1/products/" + productId + "/reviews?tier=5&page=2&size=2"),
            ReviewBreakdown.class);
        assertThat(page2.getBody().tiers().get("5").sample())
            .extracting("id").containsExactly(r3, r4);

        ResponseEntity<ReviewBreakdown> page3 = http.getForEntity(
            url("/api/v1/products/" + productId + "/reviews?tier=5&page=3&size=2"),
            ReviewBreakdown.class);
        assertThat(page3.getBody().tiers().get("5").sample())
            .extracting("id").containsExactly(r5);
    }

    @Test
    void unknownProductIs404() {
        UUID missing = UUID.randomUUID();
        ResponseEntity<String> resp = http.getForEntity(
            url("/api/v1/products/" + missing + "/reviews"), String.class);
        assertThat(resp.getStatusCode()).isEqualTo(HttpStatus.NOT_FOUND);
    }

    private String url(String path) {
        return "http://localhost:" + port + path;
    }

    private UUID insertProduct(String slug, String name) {
        UUID id = UUID.randomUUID();
        jdbc.update("""
            INSERT INTO products (id, slug, name, brand, category, description)
            VALUES (:id, :slug, :name, 'TestBrand', 'shelter', 'desc')
            """,
            new MapSqlParameterSource()
                .addValue("id", id).addValue("slug", slug).addValue("name", name));
        return id;
    }

    private void insertListing(UUID productId, String storeId, String storeProductId,
                               String confidence, Float storeRating, Integer storeReviewCount) {
        jdbc.update("""
            INSERT INTO store_listings
              (id, product_id, store_id, store_product_id, store_url,
               store_rating, store_review_count, match_confidence)
            VALUES (:id, :pid, :sid, :spid, :url, :rating, :rcount, :conf)
            """,
            new MapSqlParameterSource()
                .addValue("id", UUID.randomUUID())
                .addValue("pid", productId)
                .addValue("sid", storeId)
                .addValue("spid", storeProductId)
                .addValue("url", "https://example.com/" + storeProductId)
                .addValue("rating", storeRating)
                .addValue("rcount", storeReviewCount)
                .addValue("conf", confidence));
    }

    private UUID insertReview(UUID productId, String storeId, int rating,
                              boolean verified, int helpfulVotes, LocalDate date) {
        UUID id = UUID.randomUUID();
        jdbc.update("""
            INSERT INTO reviews
              (id, product_id, store_id, source_review_id, rating, title, body,
               verified_purchase, helpful_votes, review_date)
            VALUES (:id, :pid, :sid, :srid, :rating, 't', 'body of the review for endpoint test',
                    :verified, :hv, :date)
            """,
            new MapSqlParameterSource()
                .addValue("id", id)
                .addValue("pid", productId)
                .addValue("sid", storeId)
                .addValue("srid", "rid-" + id)
                .addValue("rating", rating)
                .addValue("verified", verified)
                .addValue("hv", helpfulVotes)
                .addValue("date", java.sql.Date.valueOf(date)));
        return id;
    }
}
