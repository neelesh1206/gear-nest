package io.gearnest.api.product.dto;

import java.util.List;
import java.util.Map;

public record ReviewBreakdown(
    Map<String, ReviewTier> tiers,
    int total,
    List<ReviewStoreBreakdownEntry> storeBreakdown
) {}
