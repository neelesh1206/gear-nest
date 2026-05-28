package io.gearnest.api.pricing;

import io.gearnest.api.product.dto.Store;
import org.springframework.jdbc.core.namedparam.MapSqlParameterSource;
import org.springframework.jdbc.core.namedparam.NamedParameterJdbcTemplate;
import org.springframework.stereotype.Repository;

import java.math.BigDecimal;
import java.time.OffsetDateTime;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

@Repository
public class PricingRepository {

    private final NamedParameterJdbcTemplate jdbc;

    public PricingRepository(NamedParameterJdbcTemplate jdbc) {
        this.jdbc = jdbc;
    }

    public List<StaticListing> listingsForProduct(UUID productId) {
        String sql = """
            SELECT sl.id, sl.store_product_id, sl.store_url, sl.affiliate_url,
                   sl.store_rating, sl.store_review_count, sl.match_confidence,
                   s.id AS store_id, s.display_name, s.logo_url
            FROM store_listings sl
            JOIN stores s ON s.id = sl.store_id
            WHERE sl.product_id = :pid
              AND sl.match_confidence IN ('EXACT','HIGH','MEDIUM')
            """;
        return jdbc.query(sql, new MapSqlParameterSource("pid", productId), (rs, i) ->
            new StaticListing(
                (UUID) rs.getObject("id"),
                new Store(rs.getString("store_id"), rs.getString("display_name"), rs.getString("logo_url")),
                rs.getString("store_product_id"),
                rs.getString("store_url"),
                rs.getString("affiliate_url"),
                rs.getObject("store_rating") == null ? null : ((BigDecimal) rs.getObject("store_rating")).floatValue(),
                rs.getInt("store_review_count"),
                rs.getString("match_confidence")
            ));
    }

    public Map<UUID, HistoricalPrice> latestHistorical(List<UUID> listingIds) {
        if (listingIds.isEmpty()) return Map.of();
        String sql = """
            SELECT DISTINCT ON (listing_id)
                   listing_id, price, in_stock, fetched_at
            FROM price_history
            WHERE listing_id IN (:ids)
            ORDER BY listing_id, fetched_at DESC
            """;
        Map<UUID, HistoricalPrice> out = new HashMap<>();
        jdbc.query(sql, new MapSqlParameterSource("ids", listingIds), rs -> {
            out.put((UUID) rs.getObject("listing_id"),
                new HistoricalPrice(
                    ((BigDecimal) rs.getObject("price")).floatValue(),
                    rs.getObject("in_stock") == null ? null : rs.getBoolean("in_stock"),
                    rs.getObject("fetched_at", OffsetDateTime.class)
                ));
        });
        return out;
    }

    public Map<UUID, Float> lowestPricesForProducts(List<UUID> productIds) {
        if (productIds.isEmpty()) return Map.of();
        String sql = """
            SELECT sl.product_id, MIN(ph.price) AS min_price
            FROM store_listings sl
            JOIN LATERAL (
                SELECT price FROM price_history
                WHERE listing_id = sl.id
                ORDER BY fetched_at DESC
                LIMIT 1
            ) ph ON TRUE
            WHERE sl.product_id IN (:ids)
            GROUP BY sl.product_id
            """;
        Map<UUID, Float> out = new HashMap<>();
        jdbc.query(sql, new MapSqlParameterSource("ids", productIds), rs -> {
            out.put((UUID) rs.getObject("product_id"),
                ((BigDecimal) rs.getObject("min_price")).floatValue());
        });
        return out;
    }

    public record HistoricalPrice(float price, Boolean inStock, OffsetDateTime fetchedAt) {}

    public Optional<OffsetDateTime> latestFetchForProduct(UUID productId) {
        String sql = """
            SELECT MAX(ph.fetched_at) FROM price_history ph
            JOIN store_listings sl ON sl.id = ph.listing_id
            WHERE sl.product_id = :pid
            """;
        OffsetDateTime ts = jdbc.queryForObject(sql, new MapSqlParameterSource("pid", productId), OffsetDateTime.class);
        return Optional.ofNullable(ts);
    }
}
