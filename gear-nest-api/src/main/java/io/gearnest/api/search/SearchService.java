package io.gearnest.api.search;

import io.gearnest.api.embedding.EmbeddingClient;
import io.gearnest.api.pricing.PricingService;
import io.gearnest.api.product.ProductRepository;
import io.gearnest.api.product.dto.AggregateRating;
import io.gearnest.api.product.dto.ProductCard;
import io.gearnest.api.product.dto.ProductSearchResponse;
import org.springframework.stereotype.Service;

import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;
import java.util.Map;
import java.util.UUID;
import java.util.stream.Collectors;

@Service
public class SearchService {

    private final ProductRepository products;
    private final EmbeddingClient embeddings;
    private final PricingService pricing;

    public SearchService(ProductRepository products, EmbeddingClient embeddings, PricingService pricing) {
        this.products = products;
        this.embeddings = embeddings;
        this.pricing = pricing;
    }

    public ProductSearchResponse search(SearchQuery q) {
        float[] embedding = (q.query() == null || q.query().isBlank())
            ? new float[384]
            : embeddings.embed(q.query());

        int offset = (q.page() - 1) * q.size();
        List<ProductCard> raw = products.hybridSearch(q.query(), embedding, q.size(), offset);
        int total = products.countSearch(q.query(), embedding);

        List<UUID> ids = raw.stream().map(ProductCard::id).toList();
        Map<UUID, AggregateRating> ratings = products.aggregateRatings(ids);
        Map<UUID, Float> lowestPrices = pricing.lowestPrices(ids);

        List<ProductCard> enriched = raw.stream().map(p -> new ProductCard(
            p.id(), p.slug(), p.name(), p.brand(), p.category(), p.subcategory(), p.primaryImage(),
            ratings.getOrDefault(p.id(), new AggregateRating(0f, 0)),
            lowestPrices.get(p.id()),
            "USD"
        )).collect(Collectors.toCollection(ArrayList::new));

        applySort(enriched, q.sort());
        applyClientFilters(enriched, q);

        return new ProductSearchResponse(enriched, total, q.page(), q.size(), null);
    }

    private void applySort(List<ProductCard> cards, String sort) {
        if (sort == null) return;
        switch (sort) {
            case "price_asc" -> cards.sort(Comparator.comparing(
                c -> c.lowestPrice() == null ? Float.MAX_VALUE : c.lowestPrice()));
            case "rating_desc" -> cards.sort(Comparator.comparing(
                (ProductCard c) -> c.aggregateRating() == null || c.aggregateRating().average() == null
                    ? 0f : c.aggregateRating().average()).reversed());
            case "best_value" -> cards.sort(Comparator.comparing(
                (ProductCard c) -> bestValueProxy(c)).reversed());
            default -> { /* relevance: server order */ }
        }
    }

    private static float bestValueProxy(ProductCard c) {
        float rating = c.aggregateRating() == null || c.aggregateRating().average() == null
            ? 0f : c.aggregateRating().average() / 5f;
        return rating;
    }

    private void applyClientFilters(List<ProductCard> cards, SearchQuery q) {
        if (q.brand() != null && !q.brand().isBlank()) {
            List<String> brands = List.of(q.brand().toLowerCase().split(","));
            cards.removeIf(c -> !brands.contains(c.brand().toLowerCase()));
        }
        if (q.category() != null && !q.category().isBlank()) {
            cards.removeIf(c -> !q.category().equalsIgnoreCase(c.category()));
        }
        if (q.minPrice() != null) {
            cards.removeIf(c -> c.lowestPrice() == null || c.lowestPrice() < q.minPrice());
        }
        if (q.maxPrice() != null) {
            cards.removeIf(c -> c.lowestPrice() != null && c.lowestPrice() > q.maxPrice());
        }
    }
}
