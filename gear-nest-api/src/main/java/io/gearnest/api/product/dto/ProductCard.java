package io.gearnest.api.product.dto;

import java.util.UUID;

public record ProductCard(
    UUID id,
    String slug,
    String name,
    String brand,
    String category,
    String subcategory,
    String primaryImage,
    AggregateRating aggregateRating,
    Float lowestPrice,
    String currency
) {}
