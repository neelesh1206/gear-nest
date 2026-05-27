package io.gearnest.api.config;

import org.springframework.boot.context.properties.ConfigurationProperties;

@ConfigurationProperties(prefix = "gearnest.huggingface")
public record HuggingFaceProperties(
    String apiKey,
    String embeddingModel,
    String chatModel,
    String baseUrl
) {}
