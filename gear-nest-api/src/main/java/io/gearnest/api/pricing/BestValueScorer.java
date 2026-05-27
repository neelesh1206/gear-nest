package io.gearnest.api.pricing;

import io.gearnest.api.product.dto.StoreListing;
import org.springframework.stereotype.Component;

import java.time.OffsetDateTime;
import java.util.ArrayList;
import java.util.Comparator;
import java.util.List;

@Component
public class BestValueScorer {

    static final float PRICE_WEIGHT = 0.4f;
    static final float RATING_WEIGHT = 0.6f;
    static final float TIEBREAK_EPSILON = 0.02f;

    public List<StoreListing> rankAndFlag(List<StoreListing> listings) {
        if (listings.isEmpty()) return listings;

        float minPrice = Float.MAX_VALUE;
        float maxPrice = Float.MIN_VALUE;
        for (StoreListing l : listings) {
            if (l.price() == null) continue;
            if (l.price() < minPrice) minPrice = l.price();
            if (l.price() > maxPrice) maxPrice = l.price();
        }
        float priceRange = maxPrice - minPrice;

        List<StoreListing> scored = new ArrayList<>(listings.size());
        for (StoreListing l : listings) {
            float priceScore;
            if (l.price() == null) {
                priceScore = 0f;
            } else if (priceRange <= 0f) {
                priceScore = 1f;
            } else {
                priceScore = (maxPrice - l.price()) / priceRange;
            }
            float ratingScore = l.storeRating() == null ? 0.5f : l.storeRating() / 5f;
            float score = PRICE_WEIGHT * priceScore + RATING_WEIGHT * ratingScore;
            scored.add(withScore(l, score));
        }

        scored.sort((a, b) -> {
            float sa = a.bestValueScore() == null ? 0f : a.bestValueScore();
            float sb = b.bestValueScore() == null ? 0f : b.bestValueScore();
            if (Math.abs(sa - sb) < TIEBREAK_EPSILON) {
                Float pa = a.price() == null ? Float.MAX_VALUE : a.price();
                Float pb = b.price() == null ? Float.MAX_VALUE : b.price();
                return Float.compare(pa, pb);
            }
            return Float.compare(sb, sa);
        });

        if (!scored.isEmpty()) {
            scored.set(0, markBest(scored.get(0)));
        }
        scored.sort(Comparator.comparing(StoreListing::inStock, Comparator.nullsLast(Comparator.reverseOrder())));
        return scored;
    }

    private static StoreListing withScore(StoreListing l, float score) {
        return new StoreListing(
            l.id(), l.store(), l.storeProductId(), l.storeUrl(), l.affiliateUrl(),
            l.price(), l.currency(), l.inStock(), l.storeRating(), l.reviewCount(),
            l.matchConfidence(), score, l.isBestValue(),
            l.priceFetchedAt(), l.isStale()
        );
    }

    private static StoreListing markBest(StoreListing l) {
        return new StoreListing(
            l.id(), l.store(), l.storeProductId(), l.storeUrl(), l.affiliateUrl(),
            l.price(), l.currency(), l.inStock(), l.storeRating(), l.reviewCount(),
            l.matchConfidence(), l.bestValueScore(), true,
            l.priceFetchedAt(), l.isStale()
        );
    }

    public static StoreListing withFreshness(StoreListing l, OffsetDateTime fetchedAt, boolean stale) {
        return new StoreListing(
            l.id(), l.store(), l.storeProductId(), l.storeUrl(), l.affiliateUrl(),
            l.price(), l.currency(), l.inStock(), l.storeRating(), l.reviewCount(),
            l.matchConfidence(), l.bestValueScore(), l.isBestValue(),
            fetchedAt, stale
        );
    }
}
