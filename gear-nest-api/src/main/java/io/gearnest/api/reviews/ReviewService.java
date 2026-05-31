package io.gearnest.api.reviews;

import io.gearnest.api.product.dto.Review;
import io.gearnest.api.product.dto.ReviewBreakdown;
import io.gearnest.api.product.dto.ReviewStoreBreakdownEntry;
import io.gearnest.api.product.dto.ReviewTier;
import org.springframework.stereotype.Service;

import java.util.LinkedHashMap;
import java.util.List;
import java.util.Map;
import java.util.UUID;

@Service
public class ReviewService {

    private final ReviewRepository repo;

    public ReviewService(ReviewRepository repo) {
        this.repo = repo;
    }

    /// No `tier` query param — return all five tiers with `sample` capped at
    /// `sampleSize` (SPEC §13 default UI: 2 surfaced reviews per tier).
    public ReviewBreakdown breakdown(UUID productId, int sampleSize) {
        List<Review> top = repo.topPerTier(productId, sampleSize);
        Map<Integer, Integer> counts = repo.countsByTier(productId);
        int total = counts.values().stream().mapToInt(Integer::intValue).sum();
        List<ReviewStoreBreakdownEntry> stores = repo.storeBreakdown(productId);

        Map<Integer, List<Review>> samplesByTier = new LinkedHashMap<>();
        for (Review r : top) {
            samplesByTier.computeIfAbsent(r.rating(), k -> new java.util.ArrayList<>()).add(r);
        }

        Map<String, ReviewTier> tiers = new LinkedHashMap<>();
        for (int t = 5; t >= 1; t--) {
            tiers.put(String.valueOf(t), new ReviewTier(
                counts.getOrDefault(t, 0),
                samplesByTier.getOrDefault(t, List.of())));
        }
        return new ReviewBreakdown(tiers, total, stores);
    }

    /// `tier` query param — paginate within one tier; `tiers` map carries
    /// only the requested tier (others omitted). Total + store breakdown
    /// stay unconditional so the UI can show context.
    public ReviewBreakdown pageForTier(UUID productId, int tier, int page, int size) {
        int offset = Math.max(0, (page - 1) * size);
        List<Review> sample = repo.pageForTier(productId, tier, size, offset);
        Map<Integer, Integer> counts = repo.countsByTier(productId);
        int total = counts.values().stream().mapToInt(Integer::intValue).sum();
        List<ReviewStoreBreakdownEntry> stores = repo.storeBreakdown(productId);

        return new ReviewBreakdown(
            Map.of(String.valueOf(tier), new ReviewTier(counts.getOrDefault(tier, 0), sample)),
            total,
            stores);
    }
}
