package io.gearnest.api.config;

import org.springframework.boot.context.properties.ConfigurationProperties;

@ConfigurationProperties(prefix = "gearnest.rag")
public record RagProperties(
    int semanticK,
    int negativeK,
    int specK,
    int overfetch,
    float mmrLambda,
    int firstTokenTimeoutSeconds
) {}
