package io.gearnest.api.rag;

import java.util.UUID;

public record Chunk(
    UUID id,
    UUID productId,
    String text,
    float[] embedding,
    Integer rating,
    String storeId,
    Source source
) {
    public enum Source { POSITIVE_REVIEW, CRITICAL_REVIEW, SPEC }
}
