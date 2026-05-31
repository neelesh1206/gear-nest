package io.gearnest.api.reviews;

import io.gearnest.api.error.NotFoundException;
import io.gearnest.api.product.ProductRepository;
import io.gearnest.api.product.dto.ReviewBreakdown;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.UUID;

@RestController
@RequestMapping("/api/v1/products")
public class ReviewController {

    private final ReviewService reviews;
    private final ProductRepository products;

    public ReviewController(ReviewService reviews, ProductRepository products) {
        this.reviews = reviews;
        this.products = products;
    }

    @GetMapping("/{id}/reviews")
    public ReviewBreakdown reviews(
        @PathVariable UUID id,
        @RequestParam(required = false) Integer tier,
        @RequestParam(defaultValue = "1") int page,
        @RequestParam(defaultValue = "2") int size
    ) {
        if (products.findNameById(id).isEmpty()) {
            throw new NotFoundException("Product not found: " + id);
        }
        int clampedSize = Math.min(Math.max(size, 1), 20);
        if (tier == null) {
            return reviews.breakdown(id, clampedSize);
        }
        return reviews.pageForTier(id, tier, Math.max(page, 1), clampedSize);
    }
}
