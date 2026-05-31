package io.gearnest.api.product.dto;

import java.util.List;

public record ReviewTier(int count, List<Review> sample) {}
