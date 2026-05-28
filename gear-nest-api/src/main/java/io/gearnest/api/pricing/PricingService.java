package io.gearnest.api.pricing;

import com.fasterxml.jackson.core.JsonProcessingException;
import com.fasterxml.jackson.databind.JsonNode;
import com.fasterxml.jackson.databind.ObjectMapper;
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

    // Must match the pipeline's staleness rule (gear-nest-pipeline/src/prices/mod.rs):
    // a snapshot is stale when now - fetched_at > 24h + jitter_secs. The per-key
    // jitter is read from the payload so reader and writer agree exactly.
    static final Duration STALE_BASE = Duration.ofHours(24);
    static final Duration NEXT_UPDATE_WINDOW = Duration.ofHours(24);

    private final PricingRepository repo;
    private final StringRedisTemplate redis;
    private final BestValueScorer scorer;
    private final ObjectMapper json;

    public PricingService(PricingRepository repo, StringRedisTemplate redis, BestValueScorer scorer, ObjectMapper json) {
        this.repo = repo;
        this.redis = redis;
        this.scorer = scorer;
        this.json = json;
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
            PriceSnapshot snap = readSnapshot(productId, s.store().id());
            Float price;
            Boolean inStock;
            OffsetDateTime fetchedAt;
            boolean stale;

            if (snap != null) {
                price = snap.price();
                inStock = snap.inStock();
                fetchedAt = snap.fetchedAt();
                stale = fetchedAt != null && fetchedAt.isBefore(
                    OffsetDateTime.now(ZoneOffset.UTC).minus(STALE_BASE).minusSeconds(snap.jitterSecs()));
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
        for (UUID pid : productIds) {
            for (StaticListing s : repo.listingsForProduct(pid)) {
                PriceSnapshot snap = readSnapshot(pid, s.store().id());
                if (snap == null || snap.price() == null) continue;
                live.merge(pid, snap.price(), Math::min);
            }
        }
        Map<UUID, Float> fallback = repo.lowestPricesForProducts(productIds);
        for (UUID pid : productIds) {
            if (!live.containsKey(pid) && fallback.containsKey(pid)) {
                live.put(pid, fallback.get(pid));
            }
        }
        return live;
    }

    // Reads the price the pipeline writes (see docs/contracts/redis-schema.md):
    //   key   = prices:{product_id}
    //   field = {store_id}
    //   value = JSON {listing_id, price, in_stock, fetched_at, jitter_secs}
    private PriceSnapshot readSnapshot(UUID productId, String storeId) {
        String key = "prices:" + productId;
        Object raw;
        try {
            raw = redis.opsForHash().get(key, storeId);
        } catch (Exception e) {
            return null;
        }
        if (raw == null) return null;
        try {
            JsonNode node = json.readTree(raw.toString());
            Float price = node.hasNonNull("price") ? Float.parseFloat(node.get("price").asText()) : null;
            Boolean inStock = node.hasNonNull("in_stock") ? node.get("in_stock").asBoolean() : null;
            OffsetDateTime fetchedAt = node.hasNonNull("fetched_at") ? parseTime(node.get("fetched_at").asText()) : null;
            long jitterSecs = node.hasNonNull("jitter_secs") ? node.get("jitter_secs").asLong() : 0L;
            return new PriceSnapshot(price, inStock, fetchedAt, jitterSecs);
        } catch (JsonProcessingException | NumberFormatException ex) {
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

    private record PriceSnapshot(Float price, Boolean inStock, OffsetDateTime fetchedAt, long jitterSecs) {}
}
