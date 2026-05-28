package io.gearnest.api.product.dto;

import java.time.OffsetDateTime;
import java.util.List;

public record PriceComparisonResponse(
    List<StoreListing> listings,
    OffsetDateTime lastUpdated,
    OffsetDateTime nextUpdate
) {}
