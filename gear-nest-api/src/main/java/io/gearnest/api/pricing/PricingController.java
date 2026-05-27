package io.gearnest.api.pricing;

import io.gearnest.api.error.NotFoundException;
import io.gearnest.api.product.ProductRepository;
import io.gearnest.api.product.dto.PriceComparisonResponse;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RestController;

import java.util.UUID;

@RestController
@RequestMapping("/api/v1/products")
public class PricingController {

    private final PricingService pricing;
    private final ProductRepository products;

    public PricingController(PricingService pricing, ProductRepository products) {
        this.pricing = pricing;
        this.products = products;
    }

    @GetMapping("/{id}/prices")
    public PriceComparisonResponse prices(@PathVariable UUID id) {
        if (products.findNameById(id).isEmpty()) {
            throw new NotFoundException("Product not found: " + id);
        }
        return pricing.comparisonFor(id);
    }
}
