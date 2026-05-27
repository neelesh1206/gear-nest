package io.gearnest.api.rag;

import io.gearnest.api.embedding.Vectors;
import org.springframework.jdbc.core.namedparam.MapSqlParameterSource;
import org.springframework.jdbc.core.namedparam.NamedParameterJdbcTemplate;
import org.springframework.stereotype.Repository;

import java.util.List;
import java.util.UUID;

@Repository
public class ChunkRepository {

    private final NamedParameterJdbcTemplate jdbc;

    public ChunkRepository(NamedParameterJdbcTemplate jdbc) {
        this.jdbc = jdbc;
    }

    public List<Chunk> findReviewChunksByCosine(UUID productId, float[] embedding, int limit) {
        return jdbc.query("""
            SELECT id, review_id, product_id, chunk_text, embedding::text AS embedding, rating, store_id
            FROM review_chunks
            WHERE product_id = :pid
            ORDER BY embedding <=> CAST(:emb AS vector)
            LIMIT :limit
            """,
            new MapSqlParameterSource()
                .addValue("pid", productId)
                .addValue("emb", Vectors.toPgvector(embedding))
                .addValue("limit", limit),
            (rs, i) -> {
                int rating = rs.getInt("rating");
                Chunk.Source source = rs.wasNull() || rating > 2 ? Chunk.Source.POSITIVE_REVIEW : Chunk.Source.CRITICAL_REVIEW;
                return new Chunk(
                    (UUID) rs.getObject("id"),
                    (UUID) rs.getObject("product_id"),
                    rs.getString("chunk_text"),
                    parseVector(rs.getString("embedding")),
                    rs.wasNull() ? null : rating,
                    rs.getString("store_id"),
                    source
                );
            });
    }

    public List<Chunk> findNegativeReviews(UUID productId, float[] embedding, int limit) {
        return jdbc.query("""
            SELECT id, product_id, chunk_text, embedding::text AS embedding, rating, store_id
            FROM review_chunks
            WHERE product_id = :pid AND rating <= 2
            ORDER BY embedding <=> CAST(:emb AS vector)
            LIMIT :limit
            """,
            new MapSqlParameterSource()
                .addValue("pid", productId)
                .addValue("emb", Vectors.toPgvector(embedding))
                .addValue("limit", limit),
            (rs, i) -> new Chunk(
                (UUID) rs.getObject("id"),
                (UUID) rs.getObject("product_id"),
                rs.getString("chunk_text"),
                parseVector(rs.getString("embedding")),
                rs.getInt("rating"),
                rs.getString("store_id"),
                Chunk.Source.CRITICAL_REVIEW
            ));
    }

    public List<Chunk> findSpecChunks(UUID productId, float[] embedding, int limit) {
        return jdbc.query("""
            SELECT id, product_id, chunk_text, embedding::text AS embedding
            FROM spec_chunks
            WHERE product_id = :pid
            ORDER BY embedding <=> CAST(:emb AS vector)
            LIMIT :limit
            """,
            new MapSqlParameterSource()
                .addValue("pid", productId)
                .addValue("emb", Vectors.toPgvector(embedding))
                .addValue("limit", limit),
            (rs, i) -> new Chunk(
                (UUID) rs.getObject("id"),
                (UUID) rs.getObject("product_id"),
                rs.getString("chunk_text"),
                parseVector(rs.getString("embedding")),
                null,
                null,
                Chunk.Source.SPEC
            ));
    }

    private static float[] parseVector(String literal) {
        if (literal == null || literal.length() < 2) return new float[0];
        String stripped = literal.substring(1, literal.length() - 1);
        if (stripped.isEmpty()) return new float[0];
        String[] parts = stripped.split(",");
        float[] out = new float[parts.length];
        for (int i = 0; i < parts.length; i++) out[i] = Float.parseFloat(parts[i]);
        return out;
    }
}
