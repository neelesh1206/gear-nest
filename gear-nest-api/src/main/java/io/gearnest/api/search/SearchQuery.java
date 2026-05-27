package io.gearnest.api.search;

public record SearchQuery(
    String query,
    String category,
    String brand,
    Float minPrice,
    Float maxPrice,
    String sort,
    int page,
    int size
) {}
