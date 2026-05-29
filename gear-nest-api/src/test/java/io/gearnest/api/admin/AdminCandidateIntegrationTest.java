package io.gearnest.api.admin;

import io.gearnest.api.product.dto.PriceComparisonResponse;
import io.gearnest.api.support.AbstractIntegrationTest;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.web.client.TestRestTemplate;
import org.springframework.boot.test.web.server.LocalServerPort;
import org.springframework.http.HttpEntity;
import org.springframework.http.HttpHeaders;
import org.springframework.http.HttpMethod;
import org.springframework.http.HttpStatus;
import org.springframework.http.MediaType;
import org.springframework.http.ResponseEntity;
import org.springframework.jdbc.core.namedparam.MapSqlParameterSource;
import org.springframework.jdbc.core.namedparam.NamedParameterJdbcTemplate;

import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.concurrent.CompletableFuture;
import java.util.concurrent.TimeUnit;

import static org.assertj.core.api.Assertions.assertThat;

class AdminCandidateIntegrationTest extends AbstractIntegrationTest {

    @LocalServerPort int port;
    @Autowired NamedParameterJdbcTemplate jdbc;
    @Autowired TestRestTemplate http;
    @Autowired CandidateService service;

    @BeforeEach
    void clean() {
        jdbc.update("DELETE FROM ai_summaries", new MapSqlParameterSource());
        jdbc.update("DELETE FROM review_chunks", new MapSqlParameterSource());
        jdbc.update("DELETE FROM reviews", new MapSqlParameterSource());
        jdbc.update("DELETE FROM store_listings", new MapSqlParameterSource());
        jdbc.update("DELETE FROM products", new MapSqlParameterSource());
    }

    @Test
    void confirmFlipsCandidateToMediumAndAppearsInPrices() {
        UUID productId = insertProduct("msr-reactor", "MSR Reactor");
        UUID listingId = insertListing(productId, "amazon", "B000RECK", "CANDIDATE");

        ResponseEntity<Void> resp = http.exchange(
            url("/api/admin/candidates/" + listingId + "/confirm"),
            HttpMethod.POST, HttpEntity.EMPTY, Void.class);
        assertThat(resp.getStatusCode()).isEqualTo(HttpStatus.NO_CONTENT);

        String confidence = jdbc.queryForObject(
            "SELECT match_confidence FROM store_listings WHERE id = :id",
            new MapSqlParameterSource("id", listingId), String.class);
        assertThat(confidence).isEqualTo("MEDIUM");

        ResponseEntity<PriceComparisonResponse> prices = http.getForEntity(
            url("/api/v1/products/" + productId + "/prices"),
            PriceComparisonResponse.class);
        assertThat(prices.getStatusCode()).isEqualTo(HttpStatus.OK);
        assertThat(prices.getBody()).isNotNull();
        assertThat(prices.getBody().listings()).extracting("id").contains(listingId);
    }

    @Test
    void reassignRepointsChunksAndDeletesSummariesForBothProducts() throws Exception {
        UUID oldProductId = insertProduct("old-product", "Old Product");
        UUID newProductId = insertProduct("new-product", "New Product");

        UUID candidateListingId = insertListing(oldProductId, "amazon", "B0OLD", "CANDIDATE");

        UUID oldReviewId = insertReview(oldProductId, "amazon", 5, "Old store-A review");
        UUID strayReviewId = insertReview(oldProductId, "rei", 4, "Old store-B review (must NOT move)");

        insertChunk(oldReviewId, oldProductId, "amazon", "amazon chunk");
        insertChunk(strayReviewId, oldProductId, "rei", "rei chunk");

        insertSummary(oldProductId, "old summary");
        insertSummary(newProductId, "new summary");

        ResponseEntity<Map> resp = http.exchange(
            url("/api/admin/candidates/" + candidateListingId + "/reassign"),
            HttpMethod.POST,
            new HttpEntity<>(Map.of("target_product_id", newProductId.toString()), jsonHeaders()),
            Map.class);
        assertThat(resp.getStatusCode()).isEqualTo(HttpStatus.ACCEPTED);
        assertThat(resp.getBody()).isNotNull();
        String jobId = (String) resp.getBody().get("jobId");
        assertThat(jobId).isNotBlank();

        CompletableFuture<Void> future = service.jobFuture(UUID.fromString(jobId));
        assertThat(future).isNotNull();
        future.get(10, TimeUnit.SECONDS);

        UUID amazonChunkProduct = jdbc.queryForObject(
            "SELECT product_id FROM review_chunks WHERE review_id = :rid",
            new MapSqlParameterSource("rid", oldReviewId), UUID.class);
        assertThat(amazonChunkProduct).isEqualTo(newProductId);

        UUID reiChunkProduct = jdbc.queryForObject(
            "SELECT product_id FROM review_chunks WHERE review_id = :rid",
            new MapSqlParameterSource("rid", strayReviewId), UUID.class);
        assertThat(reiChunkProduct).isEqualTo(oldProductId);

        Integer summaryCount = jdbc.queryForObject(
            "SELECT COUNT(*) FROM ai_summaries WHERE product_id IN (:a, :b)",
            new MapSqlParameterSource().addValue("a", oldProductId).addValue("b", newProductId),
            Integer.class);
        assertThat(summaryCount).isZero();

        Map<String, Object> listing = jdbc.queryForMap(
            "SELECT product_id, match_confidence FROM store_listings WHERE id = :id",
            new MapSqlParameterSource("id", candidateListingId));
        assertThat(listing.get("product_id")).isEqualTo(newProductId);
        assertThat(listing.get("match_confidence")).isEqualTo("HIGH");
    }

    @Test
    void listCandidatesReturnsOnlyCandidates() {
        UUID productId = insertProduct("p", "P");
        UUID candidateId = insertListing(productId, "amazon", "B0C", "CANDIDATE");
        insertListing(productId, "rei", "REI1", "EXACT");

        ResponseEntity<List> resp = http.getForEntity(url("/api/admin/candidates"), List.class);
        assertThat(resp.getStatusCode()).isEqualTo(HttpStatus.OK);
        assertThat(resp.getBody()).hasSize(1);
        Map<String, Object> row = (Map<String, Object>) resp.getBody().get(0);
        assertThat(row.get("id")).isEqualTo(candidateId.toString());
        assertThat(row.get("matchConfidence")).isEqualTo("CANDIDATE");
    }

    private String url(String path) {
        return "http://localhost:" + port + path;
    }

    private static HttpHeaders jsonHeaders() {
        HttpHeaders h = new HttpHeaders();
        h.setContentType(MediaType.APPLICATION_JSON);
        return h;
    }

    private UUID insertProduct(String slug, String name) {
        UUID id = UUID.randomUUID();
        jdbc.update("""
            INSERT INTO products (id, slug, name, brand, category, description)
            VALUES (:id, :slug, :name, 'TestBrand', 'cat', 'desc')
            """,
            new MapSqlParameterSource().addValue("id", id).addValue("slug", slug).addValue("name", name));
        return id;
    }

    private UUID insertListing(UUID productId, String storeId, String storeProductId, String confidence) {
        UUID id = UUID.randomUUID();
        jdbc.update("""
            INSERT INTO store_listings
              (id, product_id, store_id, store_product_id, store_url, match_confidence)
            VALUES (:id, :pid, :sid, :spid, :url, :conf)
            """,
            new MapSqlParameterSource()
                .addValue("id", id).addValue("pid", productId).addValue("sid", storeId)
                .addValue("spid", storeProductId)
                .addValue("url", "https://example.com/" + storeProductId)
                .addValue("conf", confidence));
        return id;
    }

    private UUID insertReview(UUID productId, String storeId, int rating, String body) {
        UUID id = UUID.randomUUID();
        jdbc.update("""
            INSERT INTO reviews (id, product_id, store_id, rating, body)
            VALUES (:id, :pid, :sid, :rating, :body)
            """,
            new MapSqlParameterSource()
                .addValue("id", id).addValue("pid", productId).addValue("sid", storeId)
                .addValue("rating", rating).addValue("body", body));
        return id;
    }

    private void insertChunk(UUID reviewId, UUID productId, String storeId, String text) {
        String vec = "[" + "0.1,".repeat(383) + "0.1]";
        jdbc.update("""
            INSERT INTO review_chunks (review_id, product_id, chunk_text, chunk_index, embedding, rating, store_id)
            VALUES (:rid, :pid, :text, 0, CAST(:emb AS vector), 5, :sid)
            """,
            new MapSqlParameterSource()
                .addValue("rid", reviewId).addValue("pid", productId).addValue("text", text)
                .addValue("emb", vec).addValue("sid", storeId));
    }

    private void insertSummary(UUID productId, String text) {
        jdbc.update("""
            INSERT INTO ai_summaries (product_id, summary_text, review_count)
            VALUES (:pid, :text, 1)
            """,
            new MapSqlParameterSource().addValue("pid", productId).addValue("text", text));
    }
}
