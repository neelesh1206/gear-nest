package io.gearnest.api.reviews;

import io.gearnest.api.product.dto.Review;
import io.gearnest.api.product.dto.ReviewStoreBreakdownEntry;
import io.gearnest.api.product.dto.Store;
import org.springframework.jdbc.core.namedparam.MapSqlParameterSource;
import org.springframework.jdbc.core.namedparam.NamedParameterJdbcTemplate;
import org.springframework.stereotype.Repository;

import java.math.BigDecimal;
import java.sql.Date;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

@Repository
public class ReviewRepository {

    private final NamedParameterJdbcTemplate jdbc;

    public ReviewRepository(NamedParameterJdbcTemplate jdbc) {
        this.jdbc = jdbc;
    }

    /// Top `perTier` reviews for every star tier, ranked by SPEC §13:
    /// `verified_purchase DESC, helpful_votes DESC, review_date DESC NULLS LAST,
    /// id`. One round trip via `ROW_NUMBER()` rather than five separate queries.
    public List<Review> topPerTier(UUID productId, int perTier) {
        String sql = """
            SELECT id, rating, title, body, verified_purchase, helpful_votes, review_date,
                   store_id, store_display_name, store_logo_url
            FROM (
              SELECT r.id, r.rating, r.title, r.body, r.verified_purchase,
                     r.helpful_votes, r.review_date,
                     s.id AS store_id, s.display_name AS store_display_name, s.logo_url AS store_logo_url,
                     ROW_NUMBER() OVER (
                       PARTITION BY r.rating
                       ORDER BY r.verified_purchase DESC,
                                r.helpful_votes DESC,
                                r.review_date DESC NULLS LAST,
                                r.id
                     ) AS rn
              FROM reviews r
              JOIN stores s ON s.id = r.store_id
              WHERE r.product_id = :pid
            ) ranked
            WHERE rn <= :n
            ORDER BY rating DESC, rn
            """;
        return jdbc.query(sql,
            new MapSqlParameterSource().addValue("pid", productId).addValue("n", perTier),
            (rs, i) -> mapReview(rs));
    }

    /// Single-tier paginated view (SPEC §13 ranking; LIMIT/OFFSET).
    public List<Review> pageForTier(UUID productId, int tier, int size, int offset) {
        String sql = """
            SELECT r.id, r.rating, r.title, r.body, r.verified_purchase,
                   r.helpful_votes, r.review_date,
                   s.id AS store_id, s.display_name AS store_display_name, s.logo_url AS store_logo_url
            FROM reviews r
            JOIN stores s ON s.id = r.store_id
            WHERE r.product_id = :pid AND r.rating = :tier
            ORDER BY r.verified_purchase DESC,
                     r.helpful_votes DESC,
                     r.review_date DESC NULLS LAST,
                     r.id
            LIMIT :lim OFFSET :off
            """;
        return jdbc.query(sql,
            new MapSqlParameterSource()
                .addValue("pid", productId)
                .addValue("tier", tier)
                .addValue("lim", size)
                .addValue("off", offset),
            (rs, i) -> mapReview(rs));
    }

    public Map<Integer, Integer> countsByTier(UUID productId) {
        String sql = "SELECT rating, COUNT(*) AS c FROM reviews WHERE product_id = :pid GROUP BY rating";
        Map<Integer, Integer> out = new HashMap<>();
        jdbc.query(sql, new MapSqlParameterSource("pid", productId), rs -> {
            out.put(rs.getInt("rating"), rs.getInt("c"));
        });
        return out;
    }

    public int totalCount(UUID productId) {
        Integer n = jdbc.queryForObject(
            "SELECT COUNT(*) FROM reviews WHERE product_id = :pid",
            new MapSqlParameterSource("pid", productId), Integer.class);
        return n == null ? 0 : n;
    }

    /// Per-store breakdown from `store_listings` (the pipeline writes per-store
    /// aggregates there from PA-API + scrape). Excludes CANDIDATE rows per
    /// ADR-007 — same filter the pricing endpoint uses.
    public List<ReviewStoreBreakdownEntry> storeBreakdown(UUID productId) {
        String sql = """
            SELECT s.id AS store_id, s.display_name, s.logo_url,
                   sl.store_review_count, sl.store_rating
            FROM store_listings sl
            JOIN stores s ON s.id = sl.store_id
            WHERE sl.product_id = :pid
              AND sl.match_confidence IN ('EXACT','HIGH','MEDIUM')
            ORDER BY sl.store_review_count DESC, s.id
            """;
        return jdbc.query(sql,
            new MapSqlParameterSource("pid", productId),
            (rs, i) -> new ReviewStoreBreakdownEntry(
                new Store(
                    rs.getString("store_id"),
                    rs.getString("display_name"),
                    rs.getString("logo_url")),
                rs.getInt("store_review_count"),
                rs.getObject("store_rating") == null
                    ? null
                    : ((BigDecimal) rs.getObject("store_rating")).floatValue()));
    }

    private static Review mapReview(java.sql.ResultSet rs) throws java.sql.SQLException {
        Date d = rs.getDate("review_date");
        return new Review(
            (UUID) rs.getObject("id"),
            rs.getInt("rating"),
            rs.getString("title"),
            rs.getString("body"),
            rs.getBoolean("verified_purchase"),
            rs.getInt("helpful_votes"),
            d == null ? null : d.toLocalDate(),
            new Store(
                rs.getString("store_id"),
                rs.getString("store_display_name"),
                rs.getString("store_logo_url")));
    }
}
