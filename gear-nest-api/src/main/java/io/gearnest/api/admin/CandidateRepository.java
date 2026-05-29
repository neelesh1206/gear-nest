package io.gearnest.api.admin;

import io.gearnest.api.product.dto.Store;
import io.gearnest.api.product.dto.StoreListing;
import org.springframework.jdbc.core.namedparam.MapSqlParameterSource;
import org.springframework.jdbc.core.namedparam.NamedParameterJdbcTemplate;
import org.springframework.stereotype.Repository;

import java.math.BigDecimal;
import java.util.List;
import java.util.Optional;
import java.util.UUID;

@Repository
public class CandidateRepository {

    private final NamedParameterJdbcTemplate jdbc;

    public CandidateRepository(NamedParameterJdbcTemplate jdbc) {
        this.jdbc = jdbc;
    }

    public List<StoreListing> findCandidates() {
        String sql = """
            SELECT sl.id, sl.store_product_id, sl.store_url, sl.affiliate_url,
                   sl.store_rating, sl.store_review_count, sl.match_confidence,
                   s.id AS store_id, s.display_name, s.logo_url
            FROM store_listings sl
            JOIN stores s ON s.id = sl.store_id
            WHERE sl.match_confidence = 'CANDIDATE'
            ORDER BY sl.id
            """;
        return jdbc.query(sql, new MapSqlParameterSource(), (rs, i) -> new StoreListing(
            (UUID) rs.getObject("id"),
            new Store(rs.getString("store_id"), rs.getString("display_name"), rs.getString("logo_url")),
            rs.getString("store_product_id"),
            rs.getString("store_url"),
            rs.getString("affiliate_url"),
            null,
            "USD",
            null,
            rs.getObject("store_rating") == null ? null : ((BigDecimal) rs.getObject("store_rating")).floatValue(),
            rs.getInt("store_review_count"),
            rs.getString("match_confidence"),
            null,
            false,
            null,
            false
        ));
    }

    public Optional<ListingPointer> findListing(UUID listingId) {
        String sql = "SELECT product_id, store_id FROM store_listings WHERE id = :id";
        return jdbc.query(sql, new MapSqlParameterSource("id", listingId),
            (rs, i) -> new ListingPointer((UUID) rs.getObject("product_id"), rs.getString("store_id")))
            .stream().findFirst();
    }

    public boolean productExists(UUID productId) {
        Integer n = jdbc.queryForObject(
            "SELECT COUNT(*) FROM products WHERE id = :id",
            new MapSqlParameterSource("id", productId), Integer.class);
        return n != null && n > 0;
    }

    public int confirm(UUID listingId) {
        return jdbc.update(
            "UPDATE store_listings SET match_confidence = 'MEDIUM' WHERE id = :id",
            new MapSqlParameterSource("id", listingId));
    }

    public int repointChunks(UUID oldProductId, String storeId, UUID newProductId) {
        String sql = """
            UPDATE review_chunks SET product_id = :new
            WHERE review_id IN (
                SELECT id FROM reviews
                WHERE product_id = :old AND store_id = :store
            )
            """;
        return jdbc.update(sql, new MapSqlParameterSource()
            .addValue("new", newProductId)
            .addValue("old", oldProductId)
            .addValue("store", storeId));
    }

    public int reassignListing(UUID listingId, UUID newProductId) {
        return jdbc.update("""
            UPDATE store_listings
            SET product_id = :pid, match_confidence = 'HIGH'
            WHERE id = :id
            """,
            new MapSqlParameterSource()
                .addValue("pid", newProductId)
                .addValue("id", listingId));
    }

    public int deleteSummaries(UUID oldProductId, UUID newProductId) {
        return jdbc.update(
            "DELETE FROM ai_summaries WHERE product_id IN (:a, :b)",
            new MapSqlParameterSource()
                .addValue("a", oldProductId)
                .addValue("b", newProductId));
    }

    public record ListingPointer(UUID productId, String storeId) {}
}
