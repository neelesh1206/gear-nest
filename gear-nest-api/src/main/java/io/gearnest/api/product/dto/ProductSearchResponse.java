package io.gearnest.api.product.dto;

import java.util.List;

public record ProductSearchResponse(
    List<ProductCard> products,
    int total,
    int page,
    int size,
    Facets facets
) {}
