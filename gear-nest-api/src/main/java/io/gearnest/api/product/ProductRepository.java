package io.gearnest.api.product;

import com.fasterxml.jackson.core.type.TypeReference;
import com.fasterxml.jackson.databind.ObjectMapper;
import io.gearnest.api.embedding.Vectors;
import io.gearnest.api.product.dto.AggregateRating;
import io.gearnest.api.product.dto.ProductCard;
import io.gearnest.api.product.dto.ProductDetail;
import org.springframework.jdbc.core.namedparam.MapSqlParameterSource;
import org.springframework.jdbc.core.namedparam.NamedParameterJdbcTemplate;
import org.springframework.stereotype.Repository;

import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.Optional;
import java.util.UUID;

@Repository
public class ProductRepository {

    private final NamedParameterJdbcTemplate jdbc;
    private final ObjectMapper json;

    public ProductRepository(NamedParameterJdbcTemplate jdbc, ObjectMapper json) {
        this.jdbc = jdbc;
        this.json = json;
    }

    public List<ProductCard> hybridSearch(String query, float[] embedding, int limit, int offset) {
        String embeddingLiteral = Vectors.toPgvector(embedding);
        MapSqlParameterSource params = new MapSqlParameterSource()
            .addValue("query", query == null ? "" : query)
            .addValue("embedding", embeddingLiteral)
            .addValue("limit", limit)
            .addValue("offset", offset);
        String sql = """
            WITH scored AS (
                SELECT p.id, p.slug, p.name, p.brand, p.category, p.subcategory, p.primary_image,
                       MAX(0.6 * (1 - (sc.embedding <=> CAST(:embedding AS vector)))) AS vec_score,
                       MAX(0.4 * ts_rank(to_tsvector('english', p.name || ' ' || p.brand || ' ' || COALESCE(p.description, '')),
                                          plainto_tsquery('english', :query))) AS fts_score
                FROM products p
                LEFT JOIN spec_chunks sc ON sc.product_id = p.id
                WHERE (:query = '' AND TRUE)
                   OR sc.embedding <=> CAST(:embedding AS vector) < 0.6
                   OR to_tsvector('english', p.name || ' ' || p.brand || ' ' || COALESCE(p.description, ''))
                      @@ plainto_tsquery('english', :query)
                GROUP BY p.id, p.slug, p.name, p.brand, p.category, p.subcategory, p.primary_image
            )
            SELECT id, slug, name, brand, category, subcategory, primary_image,
                   (COALESCE(vec_score, 0) + COALESCE(fts_score, 0)) AS score
            FROM scored
            ORDER BY score DESC NULLS LAST
            LIMIT :limit OFFSET :offset
            """;
        return jdbc.query(sql, params, (rs, i) -> new ProductCard(
            (UUID) rs.getObject("id"),
            rs.getString("slug"),
            rs.getString("name"),
            rs.getString("brand"),
            rs.getString("category"),
            rs.getString("subcategory"),
            rs.getString("primary_image"),
            null,
            null,
            "USD"
        ));
    }

    public int countSearch(String query, float[] embedding) {
        MapSqlParameterSource params = new MapSqlParameterSource()
            .addValue("query", query == null ? "" : query)
            .addValue("embedding", Vectors.toPgvector(embedding));
        String sql = """
            SELECT COUNT(DISTINCT p.id)
            FROM products p
            LEFT JOIN spec_chunks sc ON sc.product_id = p.id
            WHERE (:query = '' AND TRUE)
               OR sc.embedding <=> CAST(:embedding AS vector) < 0.6
               OR to_tsvector('english', p.name || ' ' || p.brand || ' ' || COALESCE(p.description, ''))
                  @@ plainto_tsquery('english', :query)
            """;
        Integer c = jdbc.queryForObject(sql, params, Integer.class);
        return c == null ? 0 : c;
    }

    public Optional<ProductDetail> findBySlug(String slug) {
        String sql = """
            SELECT id, slug, name, brand, category, subcategory, description, specs, primary_image
            FROM products
            WHERE slug = :slug
            """;
        List<ProductDetail> rows = jdbc.query(sql, Map.of("slug", slug), (rs, i) -> {
            Map<String, Object> specs;
            String raw = rs.getString("specs");
            try {
                specs = raw == null ? Map.of() : json.readValue(raw, new TypeReference<Map<String, Object>>() {});
            } catch (Exception e) {
                specs = Map.of();
            }
            String image = rs.getString("primary_image");
            return new ProductDetail(
                (UUID) rs.getObject("id"),
                rs.getString("slug"),
                rs.getString("name"),
                rs.getString("brand"),
                rs.getString("category"),
                rs.getString("subcategory"),
                rs.getString("description"),
                specs,
                image == null ? List.of() : List.of(image),
                null,
                List.of(),
                null,
                null
            );
        });
        return rows.stream().findFirst();
    }

    public Optional<UUID> findIdBySlug(String slug) {
        List<UUID> rows = jdbc.query("SELECT id FROM products WHERE slug = :slug",
            Map.of("slug", slug),
            (rs, i) -> (UUID) rs.getObject("id"));
        return rows.stream().findFirst();
    }

    public Optional<String> findNameById(UUID id) {
        List<String> rows = jdbc.query("SELECT name FROM products WHERE id = :id",
            Map.of("id", id),
            (rs, i) -> rs.getString("name"));
        return rows.stream().findFirst();
    }

    public Map<UUID, AggregateRating> aggregateRatings(List<UUID> ids) {
        if (ids.isEmpty()) return Map.of();
        String sql = """
            SELECT product_id, AVG(rating)::float AS avg, COUNT(*)::int AS cnt
            FROM reviews
            WHERE product_id IN (:ids)
            GROUP BY product_id
            """;
        Map<UUID, AggregateRating> out = new HashMap<>();
        jdbc.query(sql, new MapSqlParameterSource("ids", ids), rs -> {
            out.put((UUID) rs.getObject("product_id"),
                new AggregateRating((float) rs.getDouble("avg"), rs.getInt("cnt")));
        });
        return out;
    }
}
