package io.gearnest.api.pricing;

import io.gearnest.api.product.dto.PriceComparisonResponse;
import io.gearnest.api.product.dto.StoreListing;
import org.springframework.data.redis.core.StringRedisTemplate;
import org.springframework.stereotype.Service;

import java.time.Duration;
import java.time.OffsetDateTime;
import java.time.ZoneOffset;
import java.time.format.DateTimeParseException;
import java.util.ArrayList;
import java.util.HashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

@Service
public class PricingService {

    static final Duration STALE_THRESHOLD = Duration.ofHours(25);
    static final Duration NEXT_UPDATE_WINDOW = Duration.ofHours(24);

    private final PricingRepository repo;
    private final StringRedisTemplate redis;
    private final BestValueScorer scorer;

    public PricingService(PricingRepository repo, StringRedisTemplate redis, BestValueScorer scorer) {
        this.repo = repo;
        this.redis = redis;
        this.scorer = scorer;
    }

    public PriceComparisonResponse comparisonFor(UUID productId) {
        List<StaticListing> statics = repo.listingsForProduct(productId);
        if (statics.isEmpty()) {
            return new PriceComparisonResponse(List.of(), null, null);
        }

        List<UUID> listingIds = statics.stream().map(StaticListing::id).toList();
        Map<UUID, PricingRepository.HistoricalPrice> fallback = repo.latestHistorical(listingIds);

        List<StoreListing> listings = new ArrayList<>(statics.size());
        OffsetDateTime latestFetch = null;

        for (StaticListing s : statics) {
            PriceSnapshot snap = readSnapshot(s.id());
            Float price;
            Boolean inStock;
            OffsetDateTime fetchedAt;
            boolean stale;

            if (snap != null) {
                price = snap.price();
                inStock = snap.inStock();
                fetchedAt = snap.fetchedAt();
                stale = fetchedAt != null && fetchedAt.isBefore(OffsetDateTime.now(ZoneOffset.UTC).minus(STALE_THRESHOLD));
            } else {
                PricingRepository.HistoricalPrice h = fallback.get(s.id());
                price = h == null ? null : h.price();
                inStock = h == null ? null : h.inStock();
                fetchedAt = h == null ? null : h.fetchedAt();
                stale = price != null;
            }
            if (fetchedAt != null && (latestFetch == null || fetchedAt.isAfter(latestFetch))) {
                latestFetch = fetchedAt;
            }

            listings.add(new StoreListing(
                s.id(), s.store(), s.storeProductId(), s.storeUrl(), s.affiliateUrl(),
                price, "USD", inStock, s.storeRating(), s.storeReviewCount(),
                s.matchConfidence(), null, false, fetchedAt, stale
            ));
        }

        List<StoreListing> ranked = scorer.rankAndFlag(listings);
        OffsetDateTime nextUpdate = latestFetch == null ? null : latestFetch.plus(NEXT_UPDATE_WINDOW);
        return new PriceComparisonResponse(ranked, latestFetch, nextUpdate);
    }

    public Map<UUID, Float> lowestPrices(List<UUID> productIds) {
        if (productIds.isEmpty()) return Map.of();
        Map<UUID, Float> live = new HashMap<>();
        List<UUID> listingProductMap = new ArrayList<>();
        Map<UUID, UUID> listingToProduct = new HashMap<>();
        for (UUID pid : productIds) {
            for (StaticListing s : repo.listingsForProduct(pid)) {
                listingToProduct.put(s.id(), pid);
                listingProductMap.add(s.id());
            }
        }
        for (Map.Entry<UUID, UUID> e : listingToProduct.entrySet()) {
            PriceSnapshot snap = readSnapshot(e.getKey());
            if (snap == null || snap.price() == null) continue;
            live.merge(e.getValue(), snap.price(), Math::min);
        }
        Map<UUID, Float> fallback = repo.lowestPricesForProducts(productIds);
        for (UUID pid : productIds) {
            if (!live.containsKey(pid) && fallback.containsKey(pid)) {
                live.put(pid, fallback.get(pid));
            }
        }
        return live;
    }

    private PriceSnapshot readSnapshot(UUID listingId) {
        String key = "price:listing:" + listingId;
        Map<Object, Object> hash;
        try {
            hash = redis.opsForHash().entries(key);
        } catch (Exception e) {
            return null;
        }
        if (hash == null || hash.isEmpty()) return null;
        try {
            Float price = hash.get("price") == null ? null : Float.parseFloat(hash.get("price").toString());
            Boolean inStock = hash.get("in_stock") == null ? null : Boolean.parseBoolean(hash.get("in_stock").toString());
            OffsetDateTime fetchedAt = hash.get("fetched_at") == null
                ? null : parseTime(hash.get("fetched_at").toString());
            return new PriceSnapshot(price, inStock, fetchedAt);
        } catch (NumberFormatException | DateTimeParseException ex) {
            return null;
        }
    }

    private static OffsetDateTime parseTime(String s) {
        try {
            return OffsetDateTime.parse(s);
        } catch (DateTimeParseException e) {
            return null;
        }
    }

    private record PriceSnapshot(Float price, Boolean inStock, OffsetDateTime fetchedAt) {}
}
