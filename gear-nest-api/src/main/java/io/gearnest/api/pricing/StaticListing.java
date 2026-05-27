package io.gearnest.api.pricing;

import io.gearnest.api.product.dto.Store;

import java.util.UUID;

public record StaticListing(
    UUID id,
    Store store,
    String storeProductId,
    String storeUrl,
    String affiliateUrl,
    Float storeRating,
    int storeReviewCount,
    String matchConfidence
) {}
