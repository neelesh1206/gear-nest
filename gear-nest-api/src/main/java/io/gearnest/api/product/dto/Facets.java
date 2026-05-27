package io.gearnest.api.product.dto;

import java.util.List;

public record Facets(List<FacetBucket> brands, List<FacetBucket> categories, List<FacetBucket> priceRanges) {}
