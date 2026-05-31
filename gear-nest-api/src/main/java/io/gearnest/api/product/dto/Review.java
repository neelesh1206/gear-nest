package io.gearnest.api.product.dto;

import java.time.LocalDate;
import java.util.UUID;

public record Review(
    UUID id,
    int rating,
    String title,
    String body,
    boolean verifiedPurchase,
    int helpfulVotes,
    LocalDate reviewDate,
    Store store
) {}
