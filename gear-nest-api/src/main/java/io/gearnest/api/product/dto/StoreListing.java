package io.gearnest.api.product.dto;

import java.time.OffsetDateTime;
import java.util.UUID;

public record StoreListing(
    UUID id,
    Store store,
    String storeProductId,
    String storeUrl,
    String affiliateUrl,
    Float price,
    String currency,
    Boolean inStock,
    Float storeRating,
    int reviewCount,
    String matchConfidence,
    Float bestValueScore,
    boolean isBestValue,
    OffsetDateTime priceFetchedAt,
    boolean isStale
) {}
