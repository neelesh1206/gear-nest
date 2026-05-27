package io.gearnest.api.rag;

import io.gearnest.api.support.AbstractIntegrationTest;
import io.gearnest.api.support.TestStubs;
import org.junit.jupiter.api.BeforeEach;
import org.junit.jupiter.api.Test;
import org.springframework.beans.factory.annotation.Autowired;
import org.springframework.boot.test.context.SpringBootTest;
import org.springframework.boot.test.web.server.LocalServerPort;
import org.springframework.context.annotation.Import;
import org.springframework.jdbc.core.namedparam.MapSqlParameterSource;
import org.springframework.jdbc.core.namedparam.NamedParameterJdbcTemplate;

import java.net.URI;
import java.net.http.HttpClient;
import java.net.http.HttpRequest;
import java.net.http.HttpResponse;
import java.time.Duration;
import java.util.UUID;

import static org.assertj.core.api.Assertions.assertThat;

@SpringBootTest(webEnvironment = SpringBootTest.WebEnvironment.RANDOM_PORT)
@Import(TestStubs.class)
class ChatStreamIntegrationTest extends AbstractIntegrationTest {

    @LocalServerPort
    int port;

    @Autowired
    NamedParameterJdbcTemplate jdbc;

    private UUID productId;

    @BeforeEach
    void seed() {
        jdbc.update("DELETE FROM spec_chunks", new MapSqlParameterSource());
        jdbc.update("DELETE FROM review_chunks", new MapSqlParameterSource());
        jdbc.update("DELETE FROM reviews", new MapSqlParameterSource());
        jdbc.update("DELETE FROM store_listings", new MapSqlParameterSource());
        jdbc.update("DELETE FROM products", new MapSqlParameterSource());

        productId = UUID.randomUUID();
        jdbc.update("""
            INSERT INTO products (id, slug, name, brand, category, description)
            VALUES (:id, :slug, :name, :brand, :cat, :desc)
            """,
            new MapSqlParameterSource()
                .addValue("id", productId)
                .addValue("slug", "msr-reactor-stove")
                .addValue("name", TestStubs.SEEDED_PRODUCT_NAME)
                .addValue("brand", "MSR")
                .addValue("cat", "stove")
                .addValue("desc", "High-output canister stove for high altitude."));

        String vec = "[" + "0.1,".repeat(383) + "0.1]";
        jdbc.update("""
            INSERT INTO spec_chunks (product_id, chunk_text, chunk_index, source_type, embedding)
            VALUES (:pid, :text, 0, 'description', CAST(:emb AS vector))
            """,
            new MapSqlParameterSource()
                .addValue("pid", productId)
                .addValue("text", "Boils 1L in 3:30 at sea level; 8000 BTU.")
                .addValue("emb", vec));

        UUID reviewId = UUID.randomUUID();
        jdbc.update("""
            INSERT INTO reviews (id, product_id, store_id, rating, body)
            VALUES (:rid, :pid, 'amazon', 5, 'Worked great at altitude.')
            """,
            new MapSqlParameterSource()
                .addValue("rid", reviewId)
                .addValue("pid", productId));

        jdbc.update("""
            INSERT INTO review_chunks (review_id, product_id, chunk_text, chunk_index, embedding, rating, store_id)
            VALUES (:rid, :pid, :text, 0, CAST(:emb AS vector), 5, 'amazon')
            """,
            new MapSqlParameterSource()
                .addValue("rid", reviewId)
                .addValue("pid", productId)
                .addValue("text", "Worked great at altitude.")
                .addValue("emb", vec));
    }

    @Test
    void sseStreamContainsProductName() throws Exception {
        HttpClient client = HttpClient.newBuilder()
            .connectTimeout(Duration.ofSeconds(5))
            .build();
        HttpRequest req = HttpRequest.newBuilder()
            .uri(URI.create("http://localhost:" + port + "/api/v1/chat?query=performance&productId=" + productId))
            .timeout(Duration.ofSeconds(30))
            .GET()
            .build();
        HttpResponse<String> resp = client.send(req, HttpResponse.BodyHandlers.ofString());

        assertThat(resp.statusCode()).isEqualTo(200);
        assertThat(resp.headers().firstValue("Content-Type").orElse(""))
            .contains("text/event-stream");
        assertThat(resp.body()).contains(TestStubs.SEEDED_PRODUCT_NAME);
        assertThat(resp.body()).contains("event:token");
        assertThat(resp.body()).contains("event:done");
    }
}
