package io.gearnest.api.config;

import org.springframework.boot.context.properties.ConfigurationProperties;

@ConfigurationProperties(prefix = "gearnest.session")
public record SessionProperties(
    int questionLimit,
    int ttlHours,
    int inflightTtlSeconds
) {}
