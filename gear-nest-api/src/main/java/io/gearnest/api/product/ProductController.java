package io.gearnest.api.product;

import io.gearnest.api.error.NotFoundException;
import io.gearnest.api.pricing.PricingService;
import io.gearnest.api.product.dto.AggregateRating;
import io.gearnest.api.product.dto.PriceComparisonResponse;
import io.gearnest.api.product.dto.ProductDetail;
import io.gearnest.api.product.dto.ProductSearchResponse;
import io.gearnest.api.search.SearchQuery;
import io.gearnest.api.search.SearchService;
import org.springframework.web.bind.annotation.GetMapping;
import org.springframework.web.bind.annotation.PathVariable;
import org.springframework.web.bind.annotation.RequestMapping;
import org.springframework.web.bind.annotation.RequestParam;
import org.springframework.web.bind.annotation.RestController;

import java.util.Map;
import java.util.UUID;

@RestController
@RequestMapping("/api/v1/products")
public class ProductController {

    private final SearchService searchService;
    private final ProductRepository products;
    private final PricingService pricing;

    public ProductController(SearchService searchService, ProductRepository products, PricingService pricing) {
        this.searchService = searchService;
        this.products = products;
        this.pricing = pricing;
    }

    @GetMapping("/search")
    public ProductSearchResponse search(
        @RequestParam(required = false) String q,
        @RequestParam(required = false) String category,
        @RequestParam(required = false) String brand,
        @RequestParam(name = "min_price", required = false) Float minPrice,
        @RequestParam(name = "max_price", required = false) Float maxPrice,
        @RequestParam(required = false, defaultValue = "relevance") String sort,
        @RequestParam(required = false, defaultValue = "1") int page,
        @RequestParam(required = false, defaultValue = "24") int size
    ) {
        return searchService.search(new SearchQuery(q, category, brand, minPrice, maxPrice, sort, page, size));
    }

    @GetMapping("/{slug}")
    public ProductDetail detail(@PathVariable String slug) {
        ProductDetail base = products.findBySlug(slug)
            .orElseThrow(() -> new NotFoundException("Product not found: " + slug));

        PriceComparisonResponse prices = pricing.comparisonFor(base.id());
        Map<UUID, AggregateRating> ratings = products.aggregateRatings(java.util.List.of(base.id()));

        return new ProductDetail(
            base.id(), base.slug(), base.name(), base.brand(), base.category(), base.subcategory(),
            base.description(), base.specs(), base.images(),
            ratings.getOrDefault(base.id(), new AggregateRating(0f, 0)),
            prices.listings(), prices.lastUpdated(), prices.nextUpdate()
        );
    }
}
