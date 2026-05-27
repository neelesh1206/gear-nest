package io.gearnest.api.embedding;

public interface EmbeddingClient {
    float[] embed(String text);
}
