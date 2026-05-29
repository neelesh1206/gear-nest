package io.gearnest.api.admin;

import org.springframework.scheduling.annotation.Async;
import org.springframework.stereotype.Component;
import org.springframework.transaction.annotation.Transactional;

import java.util.UUID;
import java.util.concurrent.CompletableFuture;

@Component
public class CandidateReassignWorker {

    private final CandidateRepository repo;

    public CandidateReassignWorker(CandidateRepository repo) {
        this.repo = repo;
    }

    @Async
    @Transactional
    public CompletableFuture<Void> run(UUID listingId,
                                       CandidateRepository.ListingPointer pointer,
                                       UUID newProductId) {
        repo.repointChunks(pointer.productId(), pointer.storeId(), newProductId);
        repo.reassignListing(listingId, newProductId);
        repo.deleteSummaries(pointer.productId(), newProductId);
        // TODO: enqueue summary re-generation for both products once that pipeline lands.
        return CompletableFuture.completedFuture(null);
    }
}
