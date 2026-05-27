package io.gearnest.api.product.dto;

import java.time.OffsetDateTime;
import java.util.List;
import java.util.Map;
import java.util.UUID;

public record ProductDetail(
    UUID id,
    String slug,
    String name,
    String brand,
    String category,
    String subcategory,
    String description,
    Map<String, Object> specs,
    List<String> images,
    AggregateRating aggregateRating,
    List<StoreListing> listings,
    OffsetDateTime pricesLastUpdated,
    OffsetDateTime pricesNextUpdate
) {}
